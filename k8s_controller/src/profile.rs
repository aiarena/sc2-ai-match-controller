use crate::arena_api::MatchInfo;

// Profiles are used to run matches in a specific Kubernetes configuration that depends on the given match.
// For example, certain competitions may require less or more time for the match to complete;
// certain bots may require more memory to run; and
// certain matches may be run with a debug profile with verbose logs for the bot author to review.

pub struct Profile {
    pub template: &'static str,
}

impl Profile {
    pub fn get(match_info: &MatchInfo) -> Self {
        let profile_name = select_profile(match_info);

        Self {
            template: load_template(profile_name),
        }
    }
}

fn select_profile(_match_info: &MatchInfo) -> &str {
    // Use the default template for all matches
    "default"
}

fn load_template(profile_name: &str) -> &'static str {
    match profile_name {
        _ => include_str!("../templates/ac-job.yaml"),
    }
}
