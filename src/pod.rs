use podman_api::opts::ContainerCreateOpts;
use podman_api::{Id, Podman};
use rand::{distributions::Alphanumeric, Rng};

pub struct Pod {
    podman: Podman,
}

impl Pod {
    pub fn new(uid: u32) -> Pod {
        Pod {
            podman: Podman::unix(format!("/run/user/{}/podman/podman.sock", uid)),
        }
    }

    pub async fn image_exists(&self, image_id: impl Into<Id>) -> podman_api::Result<bool> {
        self.podman.images().get(image_id).exists().await
    }

    fn gen_container_name() -> String {
        format!(
            "clobber_{}",
            rand::thread_rng()
                .sample_iter(&Alphanumeric)
                .take(8)
                .map(char::from)
                .collect::<String>()
        )
    }

    pub async fn create_container(&self, image_id: String) -> Result<String, String> {
        let container_id = match self
            .podman
            .containers()
            .create(
                &ContainerCreateOpts::builder()
                    .image(image_id)
                    .name(Pod::gen_container_name())
                    .remove(false)
                    .build(),
            )
            .await
        {
            Ok(body) => body.id,
            Err(e) => return Err(format!("Error creating container: {}", e)),
        };
        let container = self.podman.containers().get(container_id.clone());
        container
            .start(None) //Maybe change detach keys? not sure
            .await
            .map_err(|e| format!("Error starting container: {}", e))
            .map(|_| container_id)
    }

    pub async fn container_exists(&self, container: Id) -> podman_api::Result<bool> {
        self.podman.containers().get(container).exists().await
    }

    pub async fn container_finished(&self, container: impl Into<Id>) -> Result<bool, String> {
        let container_id = container.into();
        let ret = self
            .podman
            .containers()
            .get(container_id.clone())
            .inspect()
            .await
            .map(|i| i.state.map(|s| s.running).flatten());
        match ret {
            Ok(val) => match val {
                Some(r) => Ok(r),
                None => Err("Could not determine state".into()),
            },
            Err(e) => Err(format!(
                "Error inspecting container {:?}: {}",
                container_id, e
            )),
        }
    }
}
