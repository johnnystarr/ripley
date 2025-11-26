// API client for Ripley REST API

const API_BASE = import.meta.env.DEV ? 'http://localhost:3000/api' : '/api';

class ApiClient {
  async request(endpoint, options = {}) {
    const url = `${API_BASE}${endpoint}`;
    const config = {
      headers: {
        'Content-Type': 'application/json',
        ...options.headers,
      },
      ...options,
    };

    try {
      const response = await fetch(url, config);
      
      if (!response.ok) {
        const error = await response.json().catch(() => ({ error: 'Unknown error' }));
        throw new Error(error.error || `HTTP ${response.status}`);
      }

      return await response.json();
    } catch (error) {
      console.error(`API Error (${endpoint}):`, error);
      throw error;
    }
  }

  // Health check
  async getHealth() {
    return this.request('/health');
  }

  // Get current rip status
  async getStatus() {
    return this.request('/status');
  }

  // Get configuration
  async getConfig() {
    return this.request('/config');
  }

  // Update configuration
  async updateConfig(config) {
    return this.request('/config', {
      method: 'POST',
      body: JSON.stringify(config),
    });
  }

  // List optical drives
  async getDrives() {
    return this.request('/drives');
  }

  // Start ripping
  async startRip(params) {
    return this.request('/rip/start', {
      method: 'POST',
      body: JSON.stringify(params),
    });
  }

  // Stop ripping
  async stopRip() {
    return this.request('/rip/stop', {
      method: 'POST',
    });
  }

  // Rename files
  async renameFiles(params) {
    return this.request('/rename', {
      method: 'POST',
      body: JSON.stringify(params),
    });
  }

  // Get WebSocket URL
  getWebSocketUrl() {
    if (import.meta.env.DEV) {
      return 'ws://localhost:3000/api/ws';
    }
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    return `${protocol}//${window.location.host}/api/ws`;
  }
}

export const api = new ApiClient();
