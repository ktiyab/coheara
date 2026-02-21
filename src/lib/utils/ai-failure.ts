/**
 * Pure classifier for AI operation failures.
 *
 * Parses error messages from real AI operations (chat, document processing)
 * and classifies them into actionable failure kinds. No side effects —
 * the caller decides what to do with the classification.
 *
 * Design: separated from AiStore so it can be tested independently
 * and reused across different error-handling contexts.
 */

/** Failure kinds that map to distinct user-facing status and recovery paths. */
export type AiFailureKind =
	| 'ollama_unreachable'
	| 'model_not_found'
	| 'generation_failed'
	| 'timeout'
	| 'unrelated';

/**
 * Classify an AI operation error into a failure kind.
 *
 * @param error - The caught error from a real AI operation (chat send, document processing).
 * @returns The classified failure kind. `'unrelated'` means the error is not AI-related
 *          (e.g. session loss, DB error) and AI status should not be updated.
 */
export function classifyAiFailure(error: unknown): AiFailureKind {
	const msg = (error instanceof Error ? error.message : String(error)).toLowerCase();

	// Session / DB errors — not AI-related, don't touch AI status
	if (msg.includes('no active profile') || msg.includes('no active session')) {
		return 'unrelated';
	}

	// Ollama process not running or unreachable
	if (
		msg.includes('connection refused') ||
		msg.includes('econnrefused') ||
		msg.includes('not detected') ||
		msg.includes('failed to connect')
	) {
		return 'ollama_unreachable';
	}

	// Model removed or unavailable
	if (msg.includes('model') && (msg.includes('not found') || msg.includes('not available'))) {
		return 'model_not_found';
	}

	// Operation timed out
	if (msg.includes('timeout') || msg.includes('timed out') || msg.includes('deadline exceeded')) {
		return 'timeout';
	}

	// Default: the generation itself failed (bad response, parse error, etc.)
	return 'generation_failed';
}
