use crate::templating::{render_job_template, JobTemplateValues};
use crate::{arenaclient::Arenaclient, k8s_config::K8sConfig, profile::Profile};
use common::api::api_reference::aiarena::aiarena_api_client::AiArenaApiClient;
use k8s_openapi::api::batch::v1::Job;
use kube::{
    api::{Api, ListParams, PostParams},
    Client,
};
use std::io::BufReader;
use tokio::time::Duration;
use tracing::{error, info};

pub async fn process(settings: K8sConfig) {
    info!("Starting k8s processing");

    let arenaclients = match load_arenaclient_details(&settings.arenaclients_json_path) {
        Ok(mut list) => {
            list.truncate(settings.max_arenaclients);
            list
        }
        Err(e) => {
            error!("Error loading arenaclient JSON file. Quitting\n{:?}", e);
            tokio::time::sleep(Duration::from_secs(10)).await;
            std::process::exit(2);
        }
    };

    let client = match Client::try_default().await {
        Ok(c) => c,
        Err(e) => {
            error!("Error creating Kubernetes API client. Quitting\n{:?}", e);
            tokio::time::sleep(Duration::from_secs(10)).await;
            std::process::exit(2);
        }
    };
    let jobs: Api<Job> = Api::namespaced(client, &settings.namespace);

    loop {
        for ac in arenaclients.iter() {
            match job_exists(&jobs, &ac.name).await {
                Ok(exists) => {
                    if exists {
                        continue;
                    }

                    info!("Retrieving new match for AC {:?}", ac.name);
                    match retrieve_match(&settings, &ac).await {
                        Ok(job_data) => {
                            info!("Creating new job for AC {:?}", &ac.name);
                            if let Err(e) = jobs.create(&PostParams::default(), &job_data).await {
                                error!("Error while creating job for AC {:?}: {:?}", &ac.name, e);
                            } else {
                                info!("Created new job for AC {:?}", &ac.name);
                            }
                        }
                        Err(e) => {
                            error!(
                                "Error while retrieving match for AC {:?}: {:?}",
                                &ac.name, e
                            );

                            // Use large cooldown to not overwhelm AI Arena API
                            tokio::time::sleep(Duration::from_secs(60)).await;
                        }
                    }
                }
                Err(e) => {
                    error!(
                        "Error while checking if job exists for AC {:?}: {:?}",
                        ac.name, e
                    );
                }
            }
        }

        // Use small cooldown to not overwhelm Kubernetes API
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

async fn retrieve_match(settings: &K8sConfig, ac: &Arenaclient) -> anyhow::Result<Job> {
    let api_client = AiArenaApiClient::new(&settings.website_url, &ac.token)?;
    let new_match = api_client.get_match().await?;

    info!("Retrieved match {:?} for AC {:?}", new_match.id, ac.name);

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
        match_controller_image: format!("aiarena/arenaclient-match:{}", settings.version),
        game_controller_image: format!("aiarena/arenaclient-sc2:{}", settings.version),
        bot1_controller_image: format!("aiarena/arenaclient-bot:{}", settings.version),
        bot1_name: new_match.bot1.name.clone(),
        bot1_id: new_match.bot1.game_display_id.clone(),
        bot2_controller_image: format!("aiarena/arenaclient-bot:{}", settings.version),
        bot2_name: new_match.bot2.name.clone(),
        bot2_id: new_match.bot2.game_display_id.clone(),
    };
    let job_data = render_job_template(template, &values)?;

    Ok(job_data)
}

fn load_arenaclient_details(path: &str) -> anyhow::Result<Vec<Arenaclient>> {
    let file = std::fs::File::open(path)?;
    let reader = BufReader::new(file);
    Ok(serde_json::from_reader(reader)?)
}

async fn job_exists(jobs: &Api<Job>, ac_name: &str) -> anyhow::Result<bool> {
    let lp = ListParams::default().labels(&format!("ac-name={}", ac_name));
    let list = jobs.list(&lp).await?;
    Ok(!list.items.is_empty())
}
