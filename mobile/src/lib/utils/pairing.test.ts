// M0-02: Tests for QR pairing utility functions
import { describe, it, expect } from 'vitest';
import { parseQrData, PairingError } from './pairing.js';

describe('parseQrData', () => {
	it('parses valid QR JSON', () => {
		const json = JSON.stringify({
			v: 1,
			url: 'https://192.168.1.100:9443',
			token: 'abc123',
			cert_fp: 'sha256:deadbeef',
			pubkey: 'dGVzdHB1YmtleQ'
		});

		const result = parseQrData(json);
		expect(result).not.toBeNull();
		expect(result!.v).toBe(1);
		expect(result!.url).toBe('https://192.168.1.100:9443');
		expect(result!.token).toBe('abc123');
		expect(result!.cert_fp).toBe('sha256:deadbeef');
		expect(result!.pubkey).toBe('dGVzdHB1YmtleQ');
	});

	it('returns null for invalid JSON', () => {
		expect(parseQrData('not json')).toBeNull();
	});

	it('returns null for empty string', () => {
		expect(parseQrData('')).toBeNull();
	});

	it('returns null for missing url field', () => {
		const json = JSON.stringify({
			v: 1,
			token: 'abc123',
			cert_fp: 'sha256:deadbeef',
			pubkey: 'dGVzdHB1YmtleQ'
		});
		expect(parseQrData(json)).toBeNull();
	});

	it('returns null for missing token field', () => {
		const json = JSON.stringify({
			v: 1,
			url: 'https://192.168.1.100:9443',
			cert_fp: 'sha256:deadbeef',
			pubkey: 'dGVzdHB1YmtleQ'
		});
		expect(parseQrData(json)).toBeNull();
	});

	it('returns null for missing pubkey field', () => {
		const json = JSON.stringify({
			v: 1,
			url: 'https://192.168.1.100:9443',
			token: 'abc123',
			cert_fp: 'sha256:deadbeef'
		});
		expect(parseQrData(json)).toBeNull();
	});

	it('returns null for wrong v type', () => {
		const json = JSON.stringify({
			v: 'one',
			url: 'https://192.168.1.100:9443',
			token: 'abc123',
			cert_fp: 'sha256:deadbeef',
			pubkey: 'dGVzdHB1YmtleQ'
		});
		expect(parseQrData(json)).toBeNull();
	});

	it('returns null for array input', () => {
		expect(parseQrData('[]')).toBeNull();
	});

	it('returns null for number input', () => {
		expect(parseQrData('42')).toBeNull();
	});
});

describe('PairingError', () => {
	it('has correct name', () => {
		const err = new PairingError('test message');
		expect(err.name).toBe('PairingError');
		expect(err.message).toBe('test message');
	});

	it('is an instance of Error', () => {
		const err = new PairingError('test');
		expect(err).toBeInstanceOf(Error);
	});
});
