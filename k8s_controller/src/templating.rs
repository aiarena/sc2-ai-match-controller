use k8s_openapi::api::batch::v1::Job;

// Values to replace placeholders in the job template
pub struct JobTemplateValues {
    pub job_name: String,
    pub configmap_name: String,
    pub match_id: String,
    pub api_client: String,
    pub api_token: String,
    pub match_controller_image: String,
    pub game_controller_image: String,
    pub bot1_controller_image: String,
    pub bot1_name: String,
    pub bot1_id: String,
    pub bot2_controller_image: String,
    pub bot2_name: String,
    pub bot2_id: String,
}

// Replaces all placeholders in the job template with actual values
// and returns a parsed Kubernetes Job object ready for creation.
pub fn render_job_template(template: &str, values: &JobTemplateValues) -> anyhow::Result<Job> {
    let rendered = template
        .replace("PLACEHOLDER_JOB_NAME", &values.job_name)
        .replace("PLACEHOLDER_CONFIGMAP_NAME", &values.configmap_name)
        .replace("PLACEHOLDER_MATCH_ID", &values.match_id)
        .replace("PLACEHOLDER_API_CLIENT", &values.api_client)
        .replace("PLACEHOLDER_API_TOKEN", &values.api_token)
        .replace(
            "PLACEHOLDER_MATCH_CONTROLLER",
            &values.match_controller_image,
        )
        .replace("PLACEHOLDER_GAME_CONTROLLER", &values.game_controller_image)
        .replace("PLACEHOLDER_BOT1_CONTROLLER", &values.bot1_controller_image)
        .replace("PLACEHOLDER_BOT1_NAME", &values.bot1_name)
        .replace("PLACEHOLDER_BOT1_ID", &values.bot1_id)
        .replace("PLACEHOLDER_BOT2_CONTROLLER", &values.bot2_controller_image)
        .replace("PLACEHOLDER_BOT2_NAME", &values.bot2_name)
        .replace("PLACEHOLDER_BOT2_ID", &values.bot2_id);

    let job: Job = serde_yaml::from_str(&rendered)?;
    Ok(job)
}
