use std::collections::BTreeMap;
use std::path::Path;

use axum::extract::State;
use axum::Json;

use crate::lfd::http::dto::{ListResponse, RepoDto};
use crate::lfd::http::state::HttpState;
use crate::lfd::http::{map_store_error, run_store, ApiResult};
use crate::lfd::types::Wave;

pub async fn list_repos_handler(
    State(state): State<HttpState>,
) -> ApiResult<ListResponse<RepoDto>> {
    let waves = run_store(&state.store, move |store| store.list_waves(None))
        .await
        .map_err(map_store_error)?;

    let repos = build_repo_dtos(waves);
    Ok(Json(ListResponse::new(repos, false)))
}

fn build_repo_dtos(waves: Vec<Wave>) -> Vec<RepoDto> {
    let mut by_repo: BTreeMap<String, u32> = BTreeMap::new();

    for wave in waves {
        *by_repo.entry(wave.repo).or_insert(0) += 1;
    }

    by_repo
        .into_iter()
        .map(|(path, wave_count)| {
            let repo_name = Path::new(&path)
                .file_name()
                .and_then(|name| name.to_str())
                .filter(|name| !name.is_empty())
                .map(std::string::ToString::to_string)
                .unwrap_or_else(|| path.clone());

            RepoDto {
                object: "repo".to_string(),
                path,
                name: repo_name,
                wave_count,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lfd::id::LfdId;

    #[test]
    fn deduplicates_repo_paths_and_counts_waves() {
        let repo_a = "/tmp/repo-a".to_string();
        let repo_b = "/tmp/repo-b".to_string();

        let waves = vec![
            Wave::new(LfdId::new(), "one".to_string(), repo_a.clone()),
            Wave::new(LfdId::new(), "two".to_string(), repo_a.clone()),
            Wave::new(LfdId::new(), "three".to_string(), repo_b.clone()),
        ];

        let repos = build_repo_dtos(waves);

        assert_eq!(repos.len(), 2);
        assert_eq!(repos[0].path, repo_a);
        assert_eq!(repos[0].wave_count, 2);
        assert_eq!(repos[1].path, repo_b);
        assert_eq!(repos[1].wave_count, 1);
    }
}
