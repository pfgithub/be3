use serde::Deserialize;

const OWNER: &str = "pfgithub";
const REPO: &str = "be3";
const WORKFLOW_FILE: &str = "ci.yml";

pub(super) fn runs_url() -> String {
    format!(
        "https://api.github.com/repos/{OWNER}/{REPO}/actions/workflows/{WORKFLOW_FILE}/runs?per_page=25"
    )
}

#[cfg(target_os = "android")]
pub(super) fn artifacts_url(run_id: u64) -> String {
    format!("https://api.github.com/repos/{OWNER}/{REPO}/actions/runs/{run_id}/artifacts")
}

/// Headers the GitHub REST API expects: a User-Agent (unauthenticated requests
/// are rejected without one) and an explicit API version.
pub(super) fn api_headers() -> Vec<(&'static str, String)> {
    vec![
        ("Accept", "application/vnd.github+json".to_owned()),
        ("X-GitHub-Api-Version", "2022-11-28".to_owned()),
        (
            "User-Agent",
            "be3-block-app-version-debug-window".to_owned(),
        ),
    ]
}

#[derive(Clone, Deserialize)]
pub(super) struct WorkflowRun {
    #[cfg_attr(not(target_os = "android"), allow(dead_code))]
    pub(super) id: u64,
    pub(super) run_number: u64,
    pub(super) head_sha: String,
    pub(super) head_branch: String,
    pub(super) event: String,
    pub(super) status: String,
    pub(super) conclusion: Option<String>,
    pub(super) created_at: String,
    pub(super) html_url: String,
}

#[derive(Deserialize)]
struct WorkflowRunsResponse {
    workflow_runs: Vec<WorkflowRun>,
}

pub(super) fn parse_runs(body: &[u8]) -> Result<Vec<WorkflowRun>, String> {
    serde_json::from_slice::<WorkflowRunsResponse>(body)
        .map(|response| response.workflow_runs)
        .map_err(|error| format!("failed to parse the workflow run list: {error}"))
}
