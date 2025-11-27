import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { api } from './api';

describe('ApiClient', () => {
  beforeEach(() => {
    global.fetch = vi.fn();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('makes GET requests correctly', async () => {
    const mockResponse = { data: 'test' };
    global.fetch = vi.fn().mockResolvedValueOnce({
      ok: true,
      json: async () => mockResponse,
    });

    const result = await api.request('/test');
    expect(result).toEqual(mockResponse);
    expect(global.fetch).toHaveBeenCalledWith(
      'http://localhost:3000/api/test',
      expect.objectContaining({ method: 'GET' })
    );
  });

  it('makes POST requests correctly', async () => {
    const mockResponse = { success: true };
    const requestBody = { key: 'value' };
    
    global.fetch = vi.fn().mockResolvedValueOnce({
      ok: true,
      json: async () => mockResponse,
    });

    const result = await api.request('/test', {
      method: 'POST',
      body: JSON.stringify(requestBody),
    });

    expect(result).toEqual(mockResponse);
    expect(global.fetch).toHaveBeenCalledWith(
      'http://localhost:3000/api/test',
      expect.objectContaining({
        method: 'POST',
      })
    );
  });

  it('handles errors correctly', async () => {
    global.fetch = vi.fn().mockResolvedValueOnce({
      ok: false,
      status: 500,
      statusText: 'Internal Server Error',
      json: async () => ({ error: 'Internal Server Error' }),
    });

    await expect(api.request('/error')).rejects.toThrow();
  });

  it('fetches drives correctly', async () => {
    const mockDrives = [{ device: '/dev/disk2', name: 'CD Drive' }];
    global.fetch = vi.fn().mockResolvedValueOnce({
      ok: true,
      json: async () => mockDrives,
    });

    const result = await api.getDrives();
    expect(result).toEqual(mockDrives);
    expect(global.fetch).toHaveBeenCalledWith(
      'http://localhost:3000/api/drives',
      expect.any(Object)
    );
  });

  it('fetches logs correctly', async () => {
    const mockLogs = [{ id: 1, message: 'Test log' }];
    global.fetch = vi.fn().mockResolvedValueOnce({
      ok: true,
      json: async () => mockLogs,
    });

    const result = await api.getLogs();
    expect(result).toEqual(mockLogs);
  });
});

