//! Docker Compose projects and services, derived from container labels.
//!
//! Compose labels every container it creates (`com.docker.compose.project`,
//! `.service`, ...), so projects can be listed without the compose files. This
//! works the same for remote hosts and for Podman.

use std::collections::BTreeMap;

use super::{ContainerState, ContainerSummary};

const LABEL_WORKING_DIR: &str = "com.docker.compose.project.working_dir";
const LABEL_CONFIG_FILES: &str = "com.docker.compose.project.config_files";

/// How many of a group's containers are running
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupStatus {
    /// All containers are running
    Running,
    /// Some, but not all, are running
    Partial,
    /// None is running
    Stopped,
}

impl GroupStatus {
    fn from_counts(running: usize, total: usize) -> Self {
        if total > 0 && running == total {
            Self::Running
        } else if running > 0 {
            Self::Partial
        } else {
            Self::Stopped
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Partial => "partial",
            Self::Stopped => "stopped",
        }
    }
}

/// One service of a compose project (all of its replicas)
#[derive(Debug, Clone)]
pub struct ComposeService {
    pub name: String,
    pub image: String,
    pub running: usize,
    pub total: usize,
    pub status: GroupStatus,
    /// The container's own status text for a single container ("Up 4 hours
    /// (healthy)"), or `running/total` for scaled services
    pub detail: String,
}

/// A compose project and its services
#[derive(Debug, Clone)]
pub struct ComposeProject {
    pub name: String,
    pub working_dir: Option<String>,
    pub config_files: Option<String>,
    pub services: Vec<ComposeService>,
    pub running: usize,
    pub total: usize,
    pub status: GroupStatus,
}

/// Group containers into compose projects (sorted by name), each with its
/// services (sorted by name). Containers without a compose project label are
/// left out.
pub fn group_projects(containers: &[ContainerSummary]) -> Vec<ComposeProject> {
    let mut projects: BTreeMap<&str, Vec<&ContainerSummary>> = BTreeMap::new();
    for c in containers {
        if let Some(project) = c.compose_project.as_deref().filter(|p| !p.is_empty()) {
            projects.entry(project).or_default().push(c);
        }
    }

    projects
        .into_iter()
        .map(|(name, members)| {
            let label = |key: &str| {
                members
                    .iter()
                    .find_map(|c| c.labels.get(key))
                    .filter(|v| !v.is_empty())
                    .cloned()
            };

            let mut services: BTreeMap<&str, Vec<&ContainerSummary>> = BTreeMap::new();
            for c in &members {
                // A labelled container without a service name still belongs here
                let service = c.compose_service.as_deref().unwrap_or("-");
                services.entry(service).or_default().push(c);
            }

            let services: Vec<ComposeService> = services
                .into_iter()
                .map(|(service, containers)| {
                    let running = containers
                        .iter()
                        .filter(|c| c.state == ContainerState::Running)
                        .count();
                    let total = containers.len();
                    let detail = if total == 1 {
                        containers[0].status.clone()
                    } else {
                        format!("{running}/{total} running")
                    };
                    ComposeService {
                        name: service.to_string(),
                        image: containers[0].image.clone(),
                        running,
                        total,
                        status: GroupStatus::from_counts(running, total),
                        detail,
                    }
                })
                .collect();

            let total = members.len();
            let running = members
                .iter()
                .filter(|c| c.state == ContainerState::Running)
                .count();
            ComposeProject {
                name: name.to_string(),
                working_dir: label(LABEL_WORKING_DIR),
                config_files: label(LABEL_CONFIG_FILES),
                services,
                running,
                total,
                status: GroupStatus::from_counts(running, total),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn container(
        project: Option<&str>,
        service: Option<&str>,
        state: ContainerState,
    ) -> ContainerSummary {
        let mut c = ContainerSummary {
            state,
            status: "Up 1 hour".to_string(),
            image: format!("img-{}", service.unwrap_or("x")),
            compose_project: project.map(String::from),
            compose_service: service.map(String::from),
            ..Default::default()
        };
        if project.is_some() {
            c.labels.insert(LABEL_WORKING_DIR.into(), "/srv/app".into());
        }
        c
    }

    #[test]
    fn groups_by_project_and_service_sorted() {
        let containers = vec![
            container(Some("web"), Some("db"), ContainerState::Running),
            container(Some("web"), Some("app"), ContainerState::Running),
            container(Some("web"), Some("app"), ContainerState::Exited),
            container(Some("blog"), Some("wp"), ContainerState::Exited),
            container(None, None, ContainerState::Running),
        ];
        let projects = group_projects(&containers);

        assert_eq!(
            projects.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(),
            vec!["blog", "web"]
        );
        let web = &projects[1];
        assert_eq!((web.running, web.total), (2, 3));
        assert_eq!(web.status, GroupStatus::Partial);
        assert_eq!(web.working_dir.as_deref(), Some("/srv/app"));
        assert_eq!(
            web.services
                .iter()
                .map(|s| s.name.as_str())
                .collect::<Vec<_>>(),
            vec!["app", "db"]
        );
        let app = &web.services[0];
        assert_eq!((app.running, app.total), (1, 2));
        assert_eq!(app.status, GroupStatus::Partial);
        assert_eq!(app.detail, "1/2 running");
        assert_eq!(web.services[1].detail, "Up 1 hour"); // single container
        assert_eq!(projects[0].status, GroupStatus::Stopped);
    }

    #[test]
    fn ignores_unlabelled_containers_and_handles_missing_service() {
        assert!(group_projects(&[container(None, None, ContainerState::Running)]).is_empty());
        let p = group_projects(&[container(Some("p"), None, ContainerState::Running)]);
        assert_eq!(p[0].services[0].name, "-");
        assert_eq!(p[0].status, GroupStatus::Running);
    }
}
