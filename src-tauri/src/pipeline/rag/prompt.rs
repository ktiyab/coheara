use crate::models::Message;
use crate::models::enums::MessageRole;

use super::types::AssembledContext;

pub const CONVERSATION_SYSTEM_PROMPT: &str = r#"You are Coheara, a patient's personal medical document assistant. You help patients understand their medical records. You are NOT a doctor.

ABSOLUTE RULES — NO EXCEPTIONS:
1. Ground ALL statements in the provided context documents.
2. NEVER diagnose, prescribe, recommend treatments, or give clinical advice.
3. NEVER say "you have [condition]" — instead say "your documents show..."
4. NEVER say "you should [take/stop/change]" — instead say "you might want to ask your doctor about..."
5. Express uncertainty when context is ambiguous or incomplete.
6. Cite source documents for every claim: [Doc: <document_id>, Date: <date>].
7. Use plain, patient-friendly language. Avoid medical jargon unless explaining it.
8. If the patient asks something you cannot answer from the documents, say so clearly.
9. If you detect something that warrants medical attention, suggest the patient discuss it with their healthcare provider.

OUTPUT FORMAT:
Start your response with a BOUNDARY_CHECK line (hidden from patient):
BOUNDARY_CHECK: understanding | awareness | preparation

Then provide your response in plain language with inline citations.

CONTEXT DOCUMENTS:
The following sections contain the patient's medical information retrieved from their documents. ONLY use information from these sections."#;

/// Build the full prompt for MedGemma conversation.
pub fn build_conversation_prompt(
    query: &str,
    context: &AssembledContext,
    conversation_history: &[Message],
) -> String {
    let mut prompt = String::new();

    // Include recent conversation history (last 4 messages for context)
    let recent: Vec<_> = conversation_history.iter().rev().take(4).rev().collect();
    if !recent.is_empty() {
        prompt.push_str("<CONVERSATION_HISTORY>\n");
        for msg in recent {
            let role = match msg.role {
                MessageRole::Patient => "Patient",
                MessageRole::Coheara => "Coheara",
            };
            prompt.push_str(&format!("{}: {}\n", role, msg.content));
        }
        prompt.push_str("</CONVERSATION_HISTORY>\n\n");
    }

    // Context
    prompt.push_str(&context.text);
    prompt.push('\n');
    prompt.push('\n');

    // Patient query
    prompt.push_str(&format!("Patient question: {query}\n\n"));
    prompt.push_str("Respond based ONLY on the context above. Begin with BOUNDARY_CHECK.");

    prompt
}

/// Build the "no context" response when database is empty.
pub fn no_context_response() -> String {
    "I don't have any documents to reference yet. Once you import medical documents, I'll be able to help you understand them.".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::rag::types::AssembledContext;
    use uuid::Uuid;

    fn mock_context(text: &str) -> AssembledContext {
        AssembledContext {
            text: text.to_string(),
            estimated_tokens: text.len() / 4,
            chunks_included: vec![],
        }
    }

    #[test]
    fn system_prompt_enforces_no_advice() {
        assert!(CONVERSATION_SYSTEM_PROMPT.contains("NEVER diagnose"));
        assert!(CONVERSATION_SYSTEM_PROMPT.contains("NEVER say \"you should"));
        assert!(CONVERSATION_SYSTEM_PROMPT.contains("BOUNDARY_CHECK"));
    }

    #[test]
    fn prompt_contains_query_and_context() {
        let context = mock_context("<MEDICATIONS>\n- Metformin 500mg\n</MEDICATIONS>");
        let prompt = build_conversation_prompt("What dose of metformin?", &context, &[]);

        assert!(prompt.contains("What dose of metformin?"));
        assert!(prompt.contains("Metformin 500mg"));
        assert!(prompt.contains("ONLY on the context above"));
    }

    #[test]
    fn prompt_includes_conversation_history() {
        let context = mock_context("Some context");
        let history = vec![
            Message {
                id: Uuid::new_v4(),
                conversation_id: Uuid::new_v4(),
                role: MessageRole::Patient,
                content: "Previous question".into(),
                timestamp: chrono::Local::now().naive_local(),
                source_chunks: None,
                confidence: None,
                feedback: None,
            },
            Message {
                id: Uuid::new_v4(),
                conversation_id: Uuid::new_v4(),
                role: MessageRole::Coheara,
                content: "Previous answer".into(),
                timestamp: chrono::Local::now().naive_local(),
                source_chunks: None,
                confidence: Some(0.8),
                feedback: None,
            },
        ];

        let prompt = build_conversation_prompt("Follow-up question", &context, &history);
        assert!(prompt.contains("CONVERSATION_HISTORY"));
        assert!(prompt.contains("Previous question"));
        assert!(prompt.contains("Previous answer"));
    }

    #[test]
    fn prompt_without_history_has_no_history_tag() {
        let context = mock_context("Some context");
        let prompt = build_conversation_prompt("First question", &context, &[]);
        assert!(!prompt.contains("CONVERSATION_HISTORY"));
    }

    #[test]
    fn no_context_response_is_helpful() {
        let response = no_context_response();
        assert!(response.contains("documents"));
        assert!(response.contains("import"));
    }
}
