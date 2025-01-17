use common::models::aiarena::aiarena_match::AiArenaMatch;
use k8s_openapi::api::batch::v1::Job;
use tracing::debug;

// Profiles are used to run matches in a specific Kubernetes configuration that depends on the given match.
// For example, certain competitions may require less or more time for the match to complete;
// certain bots may require more memory to run; and
// certain matches may be run with a debug profile with verbose logs for the bot author to review.

pub struct Profile {
    pub job_descriptor: Job,
}

impl Profile {
    pub fn get(arena_match: &AiArenaMatch) -> Self {
        let profile_name = select_profile(arena_match);

        debug!("Using job profile: {:?}", profile_name);

        Self {
            job_descriptor: serde_yaml::from_str(load_template(profile_name)).unwrap(),
        }
    }
}

fn select_profile(arena_match: &AiArenaMatch) -> &str {
    match arena_match.map.name.as_str() {
        // Map "DefendersLandingAIE" activates "strict" profile.
        // This map is used until API /api/arenaclient/v2/next-match/ gives the competition for each match.
        "DefendersLandingAIE" => "strict",

        // Use the default template for all other maps
        _ => "default",
    }
}

fn load_template(profile_name: &str) -> &str {
    match profile_name {
        "strict" => include_str!("../templates/ac-job-strict.yaml"),
        _ => include_str!("../templates/ac-job.yaml"),
    }
}
