use std::{io::BufReader, time::Instant};

use crate::templating::{render_job_template, JobTemplateValues};
use crate::{arenaclient::Arenaclient, k8s_config::K8sConfig, profile::Profile};
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
    let mut arenaclients = match load_arenaclient_details(&settings.arenaclients_json_path) {
        Ok(c) => c,
        Err(e) => {
            error!("Error loading arenaclient JSON file. Quitting\n{:?}", e);
            tokio::time::sleep(Duration::from_secs(10)).await;
            std::process::exit(2);
        }
    };

    let mut last_run = Instant::now().checked_sub(interval).unwrap();
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
        let in_progress_tokens = match get_allocated_api_tokens(&jobs, &settings.job_prefix).await {
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

        let res = join(
            schedule_jobs(&settings, &jobs, &arenaclients),
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
    arenaclients: &[Arenaclient],
) -> anyhow::Result<()> {
    for ac in arenaclients
        .iter()
        .take(settings.max_arenaclients)
        .filter(|&x| !x.allocated)
    {
        let api_client = AiArenaApiClient::new(&settings.website_url, &ac.token)?;
        debug!("Getting new match for AC {:?}", ac.name);
        match api_client.get_match().await {
            Ok(new_match) => {
                let template = Profile::get(&new_match).template;

                let job_name = if settings.job_prefix.is_empty() {
                    format!("{}-{}", ac.name.replace('_', "-"), new_match.id)
                } else {
                    format!(
                        "{}-{}-{}",
                        settings.job_prefix,
                        ac.name.replace('_', "-"),
                        new_match.id
                    )
                };

                let configmap_name = if settings.job_prefix.is_empty() {
                    "arenaclient-config".to_string()
                } else {
                    format!("{}-arenaclient-config", settings.job_prefix)
                };

                let values = JobTemplateValues {
                    job_name: job_name.clone(),
                    configmap_name,
                    match_id: new_match.id.to_string(),
                    api_client: ac.name.clone(),
                    api_token: ac.token.clone(),
                    match_controller_image: format!(
                        "aiarena/arenaclient-match:{}",
                        settings.version
                    ),
                    game_controller_image: format!("aiarena/arenaclient-sc2:{}", settings.version),
                    bot1_controller_image: format!("aiarena/arenaclient-bot:{}", settings.version),
                    bot1_name: new_match.bot1.name.clone(),
                    bot1_id: new_match.bot1.game_display_id.clone(),
                    bot2_controller_image: format!("aiarena/arenaclient-bot:{}", settings.version),
                    bot2_name: new_match.bot2.name.clone(),
                    bot2_id: new_match.bot2.game_display_id.clone(),
                };

                debug!("Rendering job template for {:?}", &job_name);
                let job_data = render_job_template(template, &values)?;

                info!(
                    "Creating new job for match {:?} for AC {:?}",
                    new_match.id, &ac.name
                );
                debug!("Creating job");
                jobs.create(&PostParams::default(), &job_data).await?;
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
                                    if name.contains(&settings.job_prefix) {
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
async fn get_allocated_api_tokens(jobs: &Api<Job>, prefix: &str) -> anyhow::Result<Vec<String>> {
    let mut api_tokens = vec![];
    for job in get_all_jobs(jobs).await?.iter().filter(|j| {
        if let Some(name) = &j.metadata.name {
            name.contains(prefix)
        } else {
            false
        }
    }) {
        if let Some(status) = &job.status {
            if status.completion_time.is_none() {
                if let Ok(api_token) = get_inner_spec(job)
                    .and_then(|f| {
                        f.containers
                            .iter()
                            .find(|c| c.name == "match-controller")
                            .and_then(|x| x.env.clone())
                            .ok_or_else(|| anyhow::format_err!("Could not find container"))
                    })
                    .and_then(|x| {
                        x.iter()
                            .find(|x| x.name == "ACMATCH_API_TOKEN")
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
