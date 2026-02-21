import { describe, it, expect } from 'vitest';
import { classifyAiFailure } from './ai-failure';

describe('classifyAiFailure', () => {
	it('classifies connection refused as ollama_unreachable', () => {
		expect(classifyAiFailure(new Error('connection refused'))).toBe('ollama_unreachable');
		expect(classifyAiFailure('ECONNREFUSED')).toBe('ollama_unreachable');
		expect(classifyAiFailure(new Error('Failed to connect to Ollama'))).toBe('ollama_unreachable');
		expect(classifyAiFailure('Ollama not detected')).toBe('ollama_unreachable');
	});

	it('classifies model errors as model_not_found', () => {
		expect(classifyAiFailure(new Error('model "medgemma" not found'))).toBe('model_not_found');
		expect(classifyAiFailure('model xyz not available')).toBe('model_not_found');
	});

	it('classifies timeout errors as timeout', () => {
		expect(classifyAiFailure(new Error('request timeout'))).toBe('timeout');
		expect(classifyAiFailure('operation timed out')).toBe('timeout');
		expect(classifyAiFailure(new Error('deadline exceeded'))).toBe('timeout');
	});

	it('classifies session errors as unrelated', () => {
		expect(classifyAiFailure(new Error('No active profile session'))).toBe('unrelated');
		expect(classifyAiFailure('No active session')).toBe('unrelated');
	});

	it('classifies unknown errors as generation_failed', () => {
		expect(classifyAiFailure(new Error('unexpected token'))).toBe('generation_failed');
		expect(classifyAiFailure('JSON parse error')).toBe('generation_failed');
		expect(classifyAiFailure(42)).toBe('generation_failed');
	});
});
