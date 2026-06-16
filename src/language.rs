pub(crate) fn sanitize_language(value: &str) -> String {
    value.trim().replace('_', "-").to_ascii_lowercase()
}
