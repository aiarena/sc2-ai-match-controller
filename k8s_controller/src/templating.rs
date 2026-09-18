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
    pub bot1_args: String,
    pub bot2_controller_image: String,
    pub bot2_name: String,
    pub bot2_id: String,
    pub bot2_args: String,
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
        .replace("PLACEHOLDER_MATCH_CONTROLLER", &values.match_controller_image)
        .replace("PLACEHOLDER_GAME_CONTROLLER", &values.game_controller_image)
        .replace("PLACEHOLDER_BOT1_CONTROLLER", &values.bot1_controller_image)
        .replace("PLACEHOLDER_BOT1_NAME", &values.bot1_name)
        .replace("PLACEHOLDER_BOT1_ID", &values.bot1_id)
        .replace("PLACEHOLDER_BOT1_ARGS", &bot_args_scalar(&values.bot1_args)?)
        .replace("PLACEHOLDER_BOT2_CONTROLLER", &values.bot2_controller_image)
        .replace("PLACEHOLDER_BOT2_NAME", &values.bot2_name)
        .replace("PLACEHOLDER_BOT2_ID", &values.bot2_id)
        .replace("PLACEHOLDER_BOT2_ARGS", &bot_args_scalar(&values.bot2_args)?);

    let job: Job = serde_yml::from_str(&rendered)?;
    Ok(job)
}

// Renders one bot's requested command line as a YAML scalar, which the bot
// controller reads back out of its BOT_ARGS environment variable and splits
// there. The string is passed through unchanged; only its YAML representation
// is chosen here.
//
// Every other value substituted into the template is one we chose; this one
// comes from whoever requested the match. So it is never spliced in raw. It is
// emitted as JSON, which is a valid YAML double-quoted scalar and is always a
// single line: every character that would end the scalar or start a new node is
// escaped, so a hostile string can only ever be the contents of BOT_ARGS, never
// document structure.
//
// Single-line matters as much as escaped. The YAML serializer would render a
// string containing a newline as a multi-line scalar, which then doesn't match
// the indentation of the placeholder it replaces and fails the whole render --
// safe, but it would strand the match it belongs to.
fn bot_args_scalar(args: &str) -> anyhow::Result<String> {
    Ok(serde_json::to_string(args)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn values() -> JobTemplateValues {
        JobTemplateValues {
            job_name: "ac-1-42".to_string(),
            configmap_name: "arenaclient-config".to_string(),
            match_id: "42".to_string(),
            api_client: "ac_1".to_string(),
            api_token: "token".to_string(),
            match_controller_image: "aiarena/arenaclient-match:latest".to_string(),
            game_controller_image: "aiarena/arenaclient-sc2:latest".to_string(),
            bot1_controller_image: "aiarena/arenaclient-bot:latest".to_string(),
            bot1_name: "basic_bot".to_string(),
            bot1_id: "bot-id-1".to_string(),
            bot1_args: String::new(),
            bot2_controller_image: "aiarena/arenaclient-bot:latest".to_string(),
            bot2_name: "loser_bot".to_string(),
            bot2_id: "bot-id-2".to_string(),
            bot2_args: String::new(),
        }
    }

    fn bot_args_env(job: &Job, container_name: &str) -> String {
        let spec = job.spec.as_ref().unwrap().template.spec.as_ref().unwrap();
        let container = spec
            .init_containers
            .as_ref()
            .unwrap()
            .iter()
            .find(|c| c.name == container_name)
            .unwrap_or_else(|| panic!("no container named {container_name}"));
        container
            .env
            .as_ref()
            .unwrap()
            .iter()
            .find(|e| e.name == "BOT_ARGS")
            .unwrap_or_else(|| panic!("{container_name} has no BOT_ARGS"))
            .value
            .clone()
            .unwrap()
    }

    #[test]
    fn renders_without_bot_args() {
        let job = render_job_template(include_str!("../templates/ac-job.yaml"), &values()).unwrap();

        assert_eq!(bot_args_env(&job, "bot-controller-1"), "");
        assert_eq!(bot_args_env(&job, "bot-controller-2"), "");
    }

    #[test]
    fn passes_each_bots_command_line_through_unchanged() {
        let mut values = values();
        values.bot1_args = "--tournament=worldcup".to_string();
        values.bot2_args = r#"--tournament=worldcup --build="all in""#.to_string();

        let job = render_job_template(include_str!("../templates/ac-job.yaml"), &values).unwrap();

        assert_eq!(bot_args_env(&job, "bot-controller-1"), "--tournament=worldcup");
        assert_eq!(bot_args_env(&job, "bot-controller-2"), r#"--tournament=worldcup --build="all in""#);
    }

    #[test]
    fn bot_args_cannot_escape_into_the_document() {
        // These come from a match requester, so try to close the scalar and add
        // structure of our own. Each must survive as plain text inside BOT_ARGS.
        let hostile = "--x\"\n            - name: SNEAK\n              value: pwned\n  evil: {a: b} # --- 'x' \\ \t";

        let mut values = values();
        values.bot1_args = hostile.to_string();

        let job = render_job_template(include_str!("../templates/ac-job.yaml"), &values).unwrap();

        assert_eq!(bot_args_env(&job, "bot-controller-1"), hostile);
        assert_eq!(bot_args_env(&job, "bot-controller-2"), "");

        let spec = job.spec.as_ref().unwrap().template.spec.as_ref().unwrap();
        let bot1 = spec.init_containers.as_ref().unwrap().iter().find(|c| c.name == "bot-controller-1").unwrap();
        let env_names: Vec<&str> = bot1.env.as_ref().unwrap().iter().map(|e| e.name.as_str()).collect();
        assert!(!env_names.contains(&"SNEAK"), "hostile args injected an environment variable: {env_names:?}");
    }
}
