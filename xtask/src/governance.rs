//! Repository protection with review exceptions isolated from mandatory checks.

use anyhow::{Context, Result, ensure};
use clap::Args;
use serde_json::{Value, json};
use std::io::Write;
use std::process::{Command, Stdio};

#[derive(Debug, Args)]
pub struct GovernanceArgs {
    /// Reconcile GitHub rulesets; the default only renders the proposed configuration.
    #[arg(long)]
    apply: bool,
}

fn ruleset(name: &str, branch: &str, rules: Vec<Value>, bypass: Vec<Value>) -> Value {
    json!({"name":name,"target":"branch","enforcement":"active",
        "conditions":{"ref_name":{"include":[format!("refs/heads/{branch}")],"exclude":[]}},
        "rules":rules,"bypass_actors":bypass})
}

fn review(count: u64, conversations: bool, owners: bool) -> Value {
    json!({"type":"pull_request","parameters":{"required_approving_review_count":count,
        "dismiss_stale_reviews_on_push":count>0,"require_code_owner_review":owners,
        "require_last_push_approval":false,"required_review_thread_resolution":conversations,
        "allowed_merge_methods":["merge"]}})
}

fn proposed(policy: &Value) -> Result<Vec<Value>> {
    let owner = policy["ownerUserId"].as_u64().context("missing owner id")?;
    ensure!(owner > 0, "invalid owner id");
    let review_bypass =
        vec![json!({"actor_id":owner,"actor_type":"User","bypass_mode":"pull_request"})];
    let contributors = policy["approvedContributorIds"]
        .as_array()
        .context("missing contributors")?
        .iter()
        .map(|id| {
            ensure!(
                id.as_u64().is_some_and(|id| id > 0),
                "invalid contributor id"
            );
            Ok(json!({"actor_id":id,"actor_type":"User","bypass_mode":"always"}))
        })
        .collect::<Result<Vec<_>>>()?;
    let mut checks: Vec<Value> = policy["requiredWorkflows"]
        .as_array()
        .context("missing workflows")?
        .iter()
        .flat_map(|workflow| workflow["jobs"].as_array().into_iter().flatten())
        .map(|name| json!({"context":name,"integration_id":15368}))
        .collect();
    if let Some(additional) = policy.get("requiredChecks") {
        for check in additional
            .as_array()
            .context("requiredChecks must be an array")?
        {
            let context = check["context"].as_str().context("missing check context")?;
            let integration = check["integrationId"]
                .as_u64()
                .context("missing check provider")?;
            ensure!(
                !context.is_empty() && integration > 0,
                "invalid required check identity"
            );
            checks.push(json!({"context":context,"integration_id":integration}));
        }
    }
    let status = json!({"type":"required_status_checks","parameters":{
        "required_status_checks":checks,"strict_required_status_checks_policy":true,
        "do_not_enforce_on_create":false}});
    let mut main_status = status.clone();
    ensure!(
        !checks.is_empty()
            && checks.iter().all(|check| check["context"]
                .as_str()
                .is_some_and(|name| !name.is_empty())),
        "required check names must be nonempty strings"
    );
    let source = policy["mainSourceCheck"]
        .as_str()
        .context("missing main source check")?;
    ensure!(!source.is_empty(), "empty main source check");
    main_status["parameters"]["required_status_checks"]
        .as_array_mut()
        .context("main checks")?
        .push(json!({"context":source,"integration_id":15368}));
    let sets = vec![
        ruleset(
            "Protect main",
            "main",
            vec![
                json!({"type":"deletion"}),
                json!({"type":"non_fast_forward"}),
                review(0, true, false),
                main_status,
            ],
            vec![],
        ),
        ruleset(
            "Main independent review",
            "main",
            vec![review(1, false, true)],
            review_bypass,
        ),
        ruleset(
            "Protect dev",
            "dev",
            vec![
                json!({"type":"deletion"}),
                json!({"type":"non_fast_forward"}),
            ],
            vec![],
        ),
        ruleset(
            "Dev contributions",
            "dev",
            vec![review(1, true, true), status],
            contributors,
        ),
    ];
    Ok(sets)
}

fn api(path: &str, method: &str, body: Option<&Value>) -> Result<Value> {
    let mut command = Command::new("gh");
    command.args(["api", "--hostname", "github.com", "--method", method, path]);
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    if body.is_some() {
        command.args(["--input", "-"]).stdin(Stdio::piped());
    }
    let mut child = command.spawn()?;
    if let Some(body) = body {
        child
            .stdin
            .take()
            .context("API input")?
            .write_all(body.to_string().as_bytes())?;
    }
    let result = child.wait_with_output()?;
    ensure!(
        result.status.success(),
        "GitHub rules request failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    serde_json::from_slice(&result.stdout).context("invalid GitHub response")
}

fn contains_configuration(actual: &Value, expected: &Value) -> bool {
    match (actual, expected) {
        (Value::Object(actual), Value::Object(expected)) => expected.iter().all(|(key, value)| {
            actual
                .get(key)
                .is_some_and(|other| contains_configuration(other, value))
        }),
        (Value::Array(actual), Value::Array(expected)) => {
            actual.len() == expected.len()
                && expected.iter().all(|value| {
                    actual
                        .iter()
                        .any(|other| contains_configuration(other, value))
                })
        }
        _ => actual == expected,
    }
}

pub fn run(args: GovernanceArgs) -> Result<()> {
    let policy: Value = serde_json::from_slice(&std::fs::read(
        crate::workspace::workspace_root().join(".github/branch-policy.json"),
    )?)?;
    let sets = proposed(&policy)?;
    if !args.apply {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({"rulesets":sets}))?
        );
        return Ok(());
    }
    let user = api("user", "GET", None)?;
    ensure!(
        user["id"] == policy["ownerUserId"],
        "only the configured owner may reconcile protection"
    );
    let repository = policy["repository"]
        .as_str()
        .context("missing repository")?;
    let metadata = api(&format!("repos/{repository}"), "GET", None)?;
    ensure!(
        metadata["id"] == policy["repositoryId"],
        "repository identity does not match policy"
    );
    let path = format!("repos/{repository}/rulesets");
    let existing = api(&format!("{path}?per_page=100"), "GET", None)?;
    for set in sets {
        let matches: Vec<_> = existing
            .as_array()
            .context("missing rulesets")?
            .iter()
            .filter(|s| s["name"] == set["name"])
            .collect();
        ensure!(
            matches.len() <= 1,
            "duplicate ruleset name {}; inspect manually",
            set["name"]
        );
        let saved = if let Some(old) = matches.first() {
            api(
                &format!("{path}/{}", old["id"].as_u64().context("ruleset id")?),
                "PUT",
                Some(&set),
            )?
        } else {
            api(&path, "POST", Some(&set))?
        };
        ensure!(
            contains_configuration(&saved, &set),
            "GitHub did not retain requested rules"
        );
        println!(
            "{}",
            json!({"id":saved["id"],"name":saved["name"],"enforcement":saved["enforcement"]})
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn owner_review_and_contribution_exceptions_never_waive_main_safety() {
        let policy = json!({"ownerUserId":1,"approvedContributorIds":[1],"requiredWorkflows":[{"jobs":["ci"]}],"mainSourceCheck":"source"});
        let sets = proposed(&policy).unwrap();
        assert_eq!(sets[0]["bypass_actors"], json!([]));
        assert_eq!(sets[1]["bypass_actors"][0]["bypass_mode"], "pull_request");
        assert_eq!(sets[2]["bypass_actors"], json!([]));
        assert_eq!(sets[3]["bypass_actors"][0]["actor_type"], "User");
    }

    #[test]
    fn response_defaults_are_allowed_but_extra_bypass_actors_are_not() {
        assert!(contains_configuration(
            &json!({"count":1,"default":false}),
            &json!({"count":1})
        ));
        assert!(!contains_configuration(
            &json!([{"actor_id":1},{"actor_id":2}]),
            &json!([{"actor_id":1}])
        ));
    }

    #[test]
    fn code_scanning_summary_is_required_with_its_actual_provider() {
        let policy = json!({"ownerUserId":1,"approvedContributorIds":[1],"requiredWorkflows":[{"jobs":["ci"]}],"mainSourceCheck":"source","requiredChecks":[{"context":"CodeQL","integrationId":57789}]});
        let sets = proposed(&policy).unwrap();
        let expected = json!({"context":"CodeQL","integration_id":57789});
        for index in [0, 3] {
            let required = sets[index]["rules"]
                .as_array()
                .unwrap()
                .iter()
                .find(|rule| rule["type"] == "required_status_checks")
                .unwrap();
            assert!(
                required["parameters"]["required_status_checks"]
                    .as_array()
                    .unwrap()
                    .contains(&expected)
            );
        }
        assert_eq!(sets[0]["bypass_actors"], json!([]));
        for check in [
            json!({"context":"","integrationId":57789}),
            json!({"context":"CodeQL","integrationId":0}),
        ] {
            let mut invalid = policy.clone();
            invalid["requiredChecks"] = json!([check]);
            assert!(proposed(&invalid).is_err());
        }
    }
}
