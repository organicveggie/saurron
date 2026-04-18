pub fn audit_update(
    container_name: &str,
    container_id: &str,
    old_image_tag: &str,
    old_image_digest: &str,
    new_image_tag: &str,
    new_image_digest: &str,
) {
    tracing::info!(
        target: "saurron::audit",
        event = "update",
        container_name,
        container_id,
        old_image_tag,
        old_image_digest,
        new_image_tag,
        new_image_digest,
        outcome = "success",
    );
}

pub fn audit_rollback(
    container_name: &str,
    container_id: &str,
    attempted_image_tag: &str,
    attempted_image_digest: &str,
    restored_image_tag: &str,
    restored_image_digest: &str,
    reason: &str,
) {
    tracing::warn!(
        target: "saurron::audit",
        event = "rollback",
        container_name,
        container_id,
        attempted_image_tag,
        attempted_image_digest,
        restored_image_tag,
        restored_image_digest,
        outcome = "rollback",
        failure_reason = reason,
    );
}
