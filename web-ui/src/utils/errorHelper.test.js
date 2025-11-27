import { describe, it, expect } from 'vitest';
import { getErrorCategory, getErrorSuggestion, formatErrorMessage } from './errorHelper';

describe('errorHelper', () => {
  describe('getErrorCategory', () => {
    it('categorizes drive errors', () => {
      expect(getErrorCategory('drive not found')).toBe('drive');
      expect(getErrorCategory('device error')).toBe('drive');
      expect(getErrorCategory('disc read failed')).toBe('drive');
    });

    it('categorizes network errors', () => {
      expect(getErrorCategory('connection timeout')).toBe('network');
      expect(getErrorCategory('network error')).toBe('network');
    });

    it('categorizes permission errors', () => {
      expect(getErrorCategory('permission denied')).toBe('permission');
      expect(getErrorCategory('access denied')).toBe('permission');
    });

    it('categorizes storage errors', () => {
      expect(getErrorCategory('disk full')).toBe('storage');
      expect(getErrorCategory('no space')).toBe('storage');
    });

    it('categorizes API errors', () => {
      expect(getErrorCategory('api key invalid')).toBe('api');
      expect(getErrorCategory('authentication failed')).toBe('api');
    });

    it('returns unknown for unrecognized errors', () => {
      expect(getErrorCategory('some random error')).toBe('unknown');
    });
  });

  describe('getErrorSuggestion', () => {
    it('provides drive error suggestions', () => {
      const suggestion = getErrorSuggestion('drive not found');
      expect(suggestion).toBeDefined();
      expect(suggestion.title).toBe('Drive Issue');
      expect(suggestion.suggestions).toBeInstanceOf(Array);
      expect(suggestion.suggestions.length).toBeGreaterThan(0);
    });

    it('provides network error suggestions', () => {
      const suggestion = getErrorSuggestion('connection timeout');
      expect(suggestion).toBeDefined();
      expect(suggestion.title).toBe('Network Error');
    });

    it('provides default suggestion for unknown errors', () => {
      const suggestion = getErrorSuggestion('random error');
      expect(suggestion).toBeDefined();
      expect(suggestion.title).toBe('Unknown Error');
    });
  });

  describe('formatErrorMessage', () => {
    it('removes error prefixes', () => {
      expect(formatErrorMessage('Error: Something went wrong')).toBe('Something went wrong');
      expect(formatErrorMessage('Failed to connect')).toBe('Connect');
      expect(formatErrorMessage('Unable to read')).toBe('Read');
    });

    it('capitalizes first letter', () => {
      expect(formatErrorMessage('lowercase error')).toBe('Lowercase error');
    });

    it('handles null/undefined', () => {
      expect(formatErrorMessage(null)).toBe('An unknown error occurred');
      expect(formatErrorMessage(undefined)).toBe('An unknown error occurred');
    });
  });
});

