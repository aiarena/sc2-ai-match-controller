use std::{io::BufReader, time::Instant};

use crate::{arenaclient::Arenaclient, k8s_config::K8sConfig};
use anyhow::Context;
use common::api::api_reference::aiarena::aiarena_api_client::AiArenaApiClient;
use futures_util::future::join;
use k8s_openapi::api::core::v1::PodSpec;
use k8s_openapi::api::{batch::v1::Job, core::v1::Node};
use kube::{
    api::{Api, DeleteParams, ListParams, PostParams},
    core::ObjectList,
    Client,
};
use tokio::time::Duration;
use tracing::{debug, error, info, trace};

pub async fn process(settings: K8sConfig) {
    let interval = Duration::from_secs(settings.interval_seconds);
    let job_yaml = include_str!("../templates/ac-job.yaml");
    let mut arenaclients = match load_arenaclient_details(&settings.arenaclients_json_path) {
        Ok(c) => c,
        Err(e) => {
            error!("Error loading arenaclient JSON file. Quitting\n{:?}", e);
            tokio::time::sleep(Duration::from_secs(10)).await;
            std::process::exit(2);
        }
    };

    let mut last_run = Instant::now() - interval;
    info!("Starting k8s processing");
    loop {
        let diff = Instant::now() - last_run;

        if diff < interval {
            tokio::time::sleep(Duration::from_secs(1)).await;
            continue;
        }
        last_run = Instant::now();
        trace!("Creating client");
        let client = match Client::try_default().await {
            Ok(c) => c,
            Err(e) => {
                error!("Error creating client: {:?}", e);
                continue;
            }
        };

        let jobs: Api<Job> = Api::namespaced(client, &settings.namespace);

        trace!("Getting allocated api tokens");
        let in_progress_tokens = match get_allocated_api_tokens(&jobs).await {
            Ok(c) => c,
            Err(e) => {
                error!("Error getting allocated api tokens: {:?}", e);
                continue;
            }
        };
        arenaclients.iter_mut().for_each(|ac| ac.allocated = false);
        trace!("Marking allocated ACS ");
        for token in in_progress_tokens.iter() {
            if let Some(ac) = arenaclients.iter_mut().find(|x| x.token == *token) {
                ac.allocated = true;
            }
        }

        let mut job_data: Job = serde_yaml::from_str(job_yaml).unwrap();

        let res = join(
            schedule_jobs(&settings, &jobs, &mut job_data, &arenaclients),
            delete_old_jobs(&settings, &jobs, &arenaclients),
        )
        .await;

        if let Err(e) = res.0 {
            error!("Error while scheduling jobs: {:?}", e);
        }

        if let Err(e) = res.1 {
            error!("Error while deleting jobs: {:?}", e);
        }
    }
}

async fn schedule_jobs(
    settings: &K8sConfig,
    jobs: &Api<Job>,
    job_data: &mut Job,
    arenaclients: &[Arenaclient],
) -> anyhow::Result<()> {
    for ac in arenaclients.iter().filter(|&x| !x.allocated) {
        let api_client = AiArenaApiClient::new(&settings.website_url, &ac.token)?; //todo: change url address
        debug!("Getting new match for AC {:?}", ac.name);
        match api_client.get_match().await {
            Ok(new_match) => {
                let new_name = if settings.job_prefix.is_empty() {
                    format!("{}-{}", ac.name.replace('_', "-"), new_match.id)
                } else {
                    format!(
                        "{}-{}-{}",
                        settings.job_prefix,
                        ac.name.replace('_', "-"),
                        new_match.id
                    )
                };
                debug!("Setting job name:{:?}", &new_name);
                set_job_name(job_data, &new_name);
                debug!("Setting API token");
                set_api_token(job_data, &ac.token)?;
                debug!("Setting job labels");
                set_job_labels(job_data, &ac.name, new_match.id)?;
                debug!("Setting configmap name");
                let new_configmap_name = if settings.job_prefix.is_empty() {
                    "arenaclient-config".to_string()
                } else {
                    format!("{}-{}", settings.job_prefix, "arenaclient-config")
                };
                set_config_configmap_name(job_data, &new_configmap_name)?;
                if let Some(version) = &settings.version {
                    debug!("Setting image tags");
                    set_image_tags(
                        job_data,
                        &[
                            "proxy-controller",
                            "bot-controller-1",
                            "bot-controller-2",
                            "sc2-controller",
                        ],
                        version,
                    )?;
                }

                info!(
                    "Creating new job for match {:?} for AC {:?}",
                    new_match.id, &ac.name
                );
                debug!("Creating job");
                jobs.create(&PostParams::default(), job_data).await?;
                debug!("Job created");
            }
            Err(e) => {
                error!(
                    "Error while retrieving match from AIArena: {:?} for AC {:?}",
                    e, &ac.name
                );
                continue;
            }
        }
    }
    Ok(())
}

async fn delete_old_jobs(
    settings: &K8sConfig,
    jobs: &Api<Job>,
    arenaclients: &[Arenaclient],
) -> anyhow::Result<()> {
    match get_all_jobs(jobs).await {
        Ok(all_jobs) => {
            for job in all_jobs {
                if let Some(ac_name) = job.metadata.labels.and_then(|l| l.get("ac-name").cloned()) {
                    if arenaclients.iter().any(|x| x.name == *ac_name) {
                        if let Some(completion_time) = job.status.and_then(|s| s.completion_time) {
                            let date_diff = chrono::Utc::now() - completion_time.0;
                            if date_diff.num_minutes() > settings.old_match_delete_after_minutes {
                                if let Some(name) = job.metadata.name {
                                    debug!(
                                        "Deleting job {} with age of {} minutes",
                                        name,
                                        date_diff.num_minutes()
                                    );
                                    if let Err(e) =
                                        jobs.delete(&name, &DeleteParams::background()).await
                                    {
                                        error!("Error deleting job {}: {:?}", name, e);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        Err(e) => return Err(e),
    }
    Ok(())
}
fn load_arenaclient_details(path: &str) -> anyhow::Result<Vec<Arenaclient>> {
    let file = std::fs::File::open(path)?;
    let reader = BufReader::new(file);
    Ok(serde_json::from_reader(reader)?)
}
fn set_job_labels(job: &mut Job, ac_name: &str, match_id: u32) -> anyhow::Result<()> {
    if job.metadata.labels.is_none() {
        job.metadata.labels = Some(std::collections::BTreeMap::new());
    }

    job.metadata
        .labels
        .as_mut()
        .context("Labels are empty")?
        .insert("ac-name".to_string(), ac_name.to_string());
    job.metadata
        .labels
        .as_mut()
        .context("Labels are empty")?
        .insert("match-id".to_string(), match_id.to_string());
    Ok(())
}
#[allow(dead_code)]
fn set_image_name(job: &mut Job, container_name: &str, image_name: &str) -> anyhow::Result<()> {
    let container = mut_inner_spec(job)?
        .containers
        .iter_mut()
        .find(|x| x.name == container_name)
        .context("container not found")?;
    container.image = Some(image_name.to_string());
    Ok(())
}

fn set_image_tags(job: &mut Job, container_names: &[&str], image_tag: &str) -> anyhow::Result<()> {
    for container in mut_inner_spec(job)?.containers.iter_mut() {
        if container_names.contains(&container.name.as_str()) {
            if let Some(image_name) = container
                .image
                .as_ref()
                .and_then(|x| x.clone().split(':').map(|x| x.to_string()).next())
            {
                let new_name = format!("{}:{}", image_name, image_tag);
                container.image = Some(new_name);
            }
        }
    }
    Ok(())
}

fn set_job_name(job: &mut Job, name: &str) {
    job.metadata.name = Some(name.to_string());
}

fn set_api_token(job: &mut Job, api_token: &str) -> anyhow::Result<()> {
    let container = mut_inner_spec(job)?
        .containers
        .iter_mut()
        .find(|x| x.name == "proxy-controller")
        .context("container not found")?;

    for env in container
        .env
        .as_mut()
        .context("env for container not found")
        .iter_mut()
    {
        for env2 in env.iter_mut() {
            if env2.name == "ACPROXY_API_TOKEN" {
                env2.value = Some(api_token.to_string());
            }
        }
    }
    Ok(())
}

#[allow(dead_code)]
fn set_node(job: &mut Job, node: &str) -> anyhow::Result<()> {
    mut_inner_spec(job)?.node_name = Some(node.to_string());

    Ok(())
}
#[allow(dead_code)]
async fn get_available_nodes(client: Client) -> anyhow::Result<ObjectList<Node>> {
    let nodes: Api<Node> = Api::all(client);
    let lp = ListParams::default();
    Ok(nodes.list(&lp).await?)
}
async fn get_all_jobs(jobs: &Api<Job>) -> anyhow::Result<ObjectList<Job>> {
    let lp = ListParams::default();
    Ok(jobs.list(&lp).await?)
}
async fn get_allocated_api_tokens(jobs: &Api<Job>) -> anyhow::Result<Vec<String>> {
    let mut api_tokens = vec![];
    for job in get_all_jobs(jobs).await? {
        if let Some(status) = &job.status {
            if status.completion_time.is_none() {
                if let Ok(api_token) = get_inner_spec(&job)
                    .and_then(|f| {
                        f.containers
                            .iter()
                            .find(|c| c.name == "proxy-controller")
                            .and_then(|x| x.env.clone())
                            .ok_or_else(|| anyhow::format_err!("Could not find container"))
                    })
                    .and_then(|x| {
                        x.iter()
                            .find(|x| x.name == "ACPROXY_API_TOKEN")
                            .and_then(|x| x.value.clone())
                            .ok_or_else(|| anyhow::format_err!("Could not find api_token"))
                    })
                {
                    api_tokens.push(api_token);
                }
            }
        }
    }
    Ok(api_tokens)
}

fn get_inner_spec(job: &Job) -> anyhow::Result<&PodSpec> {
    job.spec
        .as_ref()
        .context("Spec1 is null")?
        .template
        .spec
        .as_ref()
        .context("Spec2 is null")
}

fn mut_inner_spec(job: &mut Job) -> anyhow::Result<&mut PodSpec> {
    job.spec
        .as_mut()
        .context("Spec1 is null")?
        .template
        .spec
        .as_mut()
        .context("Spec2 is null")
}

fn set_config_configmap_name(job: &mut Job, configmap_name: &str) -> anyhow::Result<()> {
    for volume in mut_inner_spec(job)?
        .volumes
        .as_mut()
        .context("volumes not found")?
        .iter_mut()
    {
        for x in volume.config_map.iter_mut() {
            if x.name == Some("placeholder".to_string()) {
                x.name = Some(configmap_name.to_string())
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::k8s_processor::{
        get_inner_spec, set_api_token, set_config_configmap_name, set_image_name, set_image_tags,
        set_job_labels, set_job_name, set_node,
    };
    use k8s_openapi::api::batch::v1::{Job, JobSpec};
    use k8s_openapi::api::core::v1::{Container, PodSpec, PodTemplateSpec};

    fn create_job_with_image_name(img_name: &str) -> Job {
        Job {
            metadata: Default::default(),
            spec: Some(JobSpec {
                template: PodTemplateSpec {
                    metadata: None,
                    spec: Some(PodSpec {
                        containers: vec![Container {
                            name: "arenaclient".to_string(),
                            image: Some(img_name.to_string()),
                            ..Default::default()
                        }],
                        ..Default::default()
                    }),
                },
                ..Default::default()
            }),
            ..Default::default()
        }
    }
    #[test]
    fn test_set_image_tags_latest_tag() {
        let mut job = create_job_with_image_name("aiarena/arenaclient:latest");
        assert!(set_image_tags(&mut job, &["arenaclient"], "test-tag").is_ok());
        assert_eq!(
            get_inner_spec(&job).unwrap().containers[0]
                .image
                .as_ref()
                .unwrap(),
            "aiarena/arenaclient:test-tag"
        );
    }

    #[test]
    fn test_set_image_tags_no_tag() {
        let mut job = create_job_with_image_name("aiarena/arenaclient");
        assert!(set_image_tags(&mut job, &["arenaclient"], "test-tag").is_ok());
        assert_eq!(
            get_inner_spec(&job).unwrap().containers[0]
                .image
                .as_ref()
                .unwrap(),
            "aiarena/arenaclient:test-tag"
        );
    }

    #[test]
    fn test_set_node_name() {
        let mut job = create_job_with_image_name("blank");
        let node_name = "node1";
        assert!(set_node(&mut job, node_name).is_ok());
        assert_eq!(
            get_inner_spec(&job).unwrap().node_name.as_ref().unwrap(),
            node_name
        );
    }
    fn load_job_from_template() -> Job {
        let job_yaml = include_str!("../templates/ac-job.yaml");
        serde_yaml::from_str(job_yaml).expect("Could not deserialize template")
    }

    #[test]
    fn test_set_job_label_is_some() {
        let mut job = load_job_from_template();
        let ac_name = "test-ac".to_string();
        let match_id = 0;
        set_job_labels(&mut job, &ac_name, 0).expect("Could not set job labels");

        assert!(job.metadata.labels.is_some());
        assert_eq!(
            job.metadata.labels.as_ref().unwrap().get("ac-name"),
            Some(&ac_name)
        );
        assert_eq!(
            job.metadata.labels.as_ref().unwrap().get("match-id"),
            Some(&match_id.to_string())
        );
    }

    #[test]
    fn test_set_image_name() {
        let mut job = load_job_from_template();
        let container_name = "proxy-controller".to_string();
        let image_name = "test-image".to_string();
        set_image_name(&mut job, &container_name, &image_name).expect("Could not set image name");

        assert_eq!(
            get_inner_spec(&job)
                .expect("Inner spec does not exist")
                .containers
                .iter()
                .find(|x| x.name == container_name)
                .expect("Could not find container")
                .image,
            Some(image_name)
        );
    }

    #[test]
    fn test_set_job_name() {
        let mut job = Job::default();
        let job_name = "test-name".to_string();
        set_job_name(&mut job, &job_name);

        assert_eq!(job.metadata.name, Some(job_name));
    }

    #[test]
    fn test_set_config_configmap_name() {
        let mut job = load_job_from_template();
        let configmap_name = "test-configmap".to_string();

        set_config_configmap_name(&mut job, &configmap_name).expect("Could not set configmap name");
        println!("{:?}", job);
        assert!(get_inner_spec(&job)
            .expect("Inner spec is null")
            .volumes
            .as_ref()
            .expect("Volumes is None")
            .iter()
            .map(|x| x.config_map.as_ref())
            .any(|x| x.and_then(|c| c.name.as_ref()) == Some(&configmap_name)))
    }

    #[test]
    fn test_set_api_token() {
        let mut job = load_job_from_template();
        let api_token = "123".to_string();
        set_api_token(&mut job, &api_token).expect("Could not set api token");

        assert_eq!(
            get_inner_spec(&job)
                .and_then(|f| {
                    f.containers
                        .iter()
                        .find(|c| c.name == "proxy-controller")
                        .and_then(|x| x.env.clone())
                        .ok_or_else(|| anyhow::format_err!("Could not find container"))
                })
                .and_then(|x| {
                    x.iter()
                        .find(|x| x.name == "ACPROXY_API_TOKEN")
                        .and_then(|x| x.value.clone())
                        .ok_or_else(|| anyhow::format_err!("Could not find api_token"))
                })
                .expect("Could not find api_token"),
            api_token
        );
    }
}
