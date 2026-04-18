pub fn bootstrap_message() -> &'static str {
    "StudyOS bootstrap: terminal-native adaptive tutor runtime"
}

#[cfg(test)]
mod tests {
    use super::bootstrap_message;

    #[test]
    fn bootstrap_message_mentions_studyos() {
        assert!(bootstrap_message().contains("StudyOS"));
    }
}
