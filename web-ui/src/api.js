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

  // Detect drives (alias for getDrives for consistency)
  async detectDrives() {
    return this.request('/drives');
  }

  // Eject a drive
  async ejectDrive(device) {
    // URL encode the device path
    const encodedDevice = encodeURIComponent(device);
    return this.request(`/drives/${encodedDevice}/eject`, {
      method: 'POST',
    });
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

  // Get recent logs
  async getLogs() {
    return this.request('/logs');
  }

  // Search logs
  async searchLogs(params) {
    const query = new URLSearchParams(params).toString();
    return this.request(`/logs/search?${query}`);
  }

  // Get all issues
  async getIssues() {
    return this.request('/issues');
  }

  // Get active (unresolved) issues
  async getActiveIssues() {
    return this.request('/issues/active');
  }

  // Resolve an issue
  async resolveIssue(issueId) {
    return this.request(`/issues/${issueId}/resolve`, {
      method: 'POST',
    });
  }

  // Get last used title
  async getLastTitle() {
    return this.request('/settings/last-title');
  }

  // Set last used title
  async setLastTitle(title) {
    return this.request('/settings/last-title', {
      method: 'POST',
      body: JSON.stringify({ title }),
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
