use std::{io::BufReader, time::Instant};

use crate::{arenaclient::Arenaclient, k8s_config::K8sConfig};
use anyhow::Context;
use common::api::api_reference::aiarena::aiarena_api_client::AiArenaApiClient;
use futures_util::future::join;
use k8s_openapi::api::{batch::v1::Job, core::v1::Node};
use kube::{
    api::{Api, DeleteParams, ListParams, PostParams},
    core::ObjectList,
    Client,
};
use tokio::time::Duration;
use tracing::{debug, error, info, trace};

// static OLD_MATCH_DELETE_AFTER_MINUTES: i64 = 10;
// static JOB_PREFIX: &str = "staging";
// static WEBSITE_URL: &str = "https://staging.aiarena.net";
// static NAMESPACE: &str = "arenaclients";
// static ARENACLIENTS_JSON_PATH: &str = "arenaclients.json";

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
                set_job_labels(job_data, &ac.name, new_match.id);
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

        // info!("Waiting for job to complete");
        // let cond = await_condition(jobs.clone(), name, conditions::is_job_completed());
        // let _ = tokio::time::timeout(std::time::Duration::from_secs(20), cond).await?;

        // info!("Cleaning up job record");
        // jobs.delete(name, &DeleteParams::background()).await?;
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
fn set_job_labels(job: &mut Job, ac_name: &str, match_id: u32) {
    if job.metadata.labels.is_none() {
        job.metadata.labels = Some(std::collections::BTreeMap::new());
    }

    job.metadata
        .labels
        .as_mut()
        .unwrap()
        .insert("ac-name".to_string(), ac_name.to_string());
    job.metadata
        .labels
        .as_mut()
        .unwrap()
        .insert("match-id".to_string(), match_id.to_string());
}
#[allow(dead_code)]
fn set_image_name(job: &mut Job, container_name: &str, image_name: &str) -> anyhow::Result<()> {
    let container = job
        .spec
        .as_mut()
        .context("spec1 not found")?
        .template
        .spec
        .as_mut()
        .context("spec2 not found")?
        .containers
        .iter_mut()
        .find(|x| x.name == container_name)
        .context("container not found")?;
    container.image = Some(image_name.to_string());
    Ok(())
}

fn set_job_name(job: &mut Job, name: &str) {
    job.metadata.name = Some(name.to_string());
}

fn set_api_token(job: &mut Job, api_token: &str) -> anyhow::Result<()> {
    let container = job
        .spec
        .as_mut()
        .context("spec1 not found")?
        .template
        .spec
        .as_mut()
        .context("spec2 not found")?
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
    job.spec
        .as_mut()
        .context("spec1 not found")?
        .template
        .spec
        .as_mut()
        .context("spec2 not found")?
        .node_name = Some(node.to_string());

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
        if let Some(status) = job.status {
            if status.completion_time.is_none() {
                if let Some(api_token) = job
                    .spec
                    .and_then(|x| {
                        x.template.spec.and_then(|f| {
                            f.containers
                                .iter()
                                .find(|c| c.name == "proxy-controller")
                                .and_then(|x| x.env.clone())
                        })
                    })
                    .and_then(|x| {
                        x.iter()
                            .find(|x| x.name == "ACPROXY_API_TOKEN")
                            .and_then(|x| x.value.clone())
                    })
                {
                    api_tokens.push(api_token);
                }
            }
        }
    }
    Ok(api_tokens)
}
