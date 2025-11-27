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

  // Get config file path
  async getConfigPath() {
    return this.request('/config/path');
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

  // Pause rip operation
  async pauseRip(drive) {
    return this.request(`/rip/${encodeURIComponent(drive)}/pause`, {
      method: 'PUT',
    });
  }

  // Resume rip operation
  async resumeRip(drive) {
    return this.request(`/rip/${encodeURIComponent(drive)}/resume`, {
      method: 'PUT',
    });
  }

  // Get rip queue
  async getQueue() {
    return this.request('/queue');
  }

  // Cancel queue entry
  async cancelQueueEntry(queueId) {
    return this.request(`/queue/${queueId}/cancel`, {
      method: 'DELETE',
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

  // Clear all logs
  async clearLogs() {
    return this.request('/logs/clear', {
      method: 'DELETE',
    });
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

  // Assign an issue
  async assignIssue(issueId, assignedTo) {
    return this.request(`/issues/${issueId}/assign`, {
      method: 'PUT',
      body: JSON.stringify({ assigned_to: assignedTo || null }),
    });
  }

  // Update resolution notes for an issue
  async updateResolutionNotes(issueId, notes) {
    return this.request(`/issues/${issueId}/resolution-notes`, {
      method: 'PUT',
      body: JSON.stringify({ notes }),
    });
  }

  // Get notes for an issue
  async getIssueNotes(issueId) {
    return this.request(`/issues/${issueId}/notes`);
  }

  // Add a note to an issue
  async addIssueNote(issueId, note) {
    return this.request(`/issues/${issueId}/notes`, {
      method: 'POST',
      body: JSON.stringify({ note }),
    });
  }

  // Delete an issue note
  async deleteIssueNote(issueId, noteId) {
    return this.request(`/issues/${issueId}/notes/${noteId}`, {
      method: 'DELETE',
    });
  }

  // Set last used title
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

  // Get last selected show ID
  async getLastShowId() {
    return this.request('/settings/last-show');
  }

  // Get all shows
  async getShows() {
    return this.request('/shows');
  }

  // Create a new show
  async createShow(name) {
    return this.request('/shows', {
      method: 'POST',
      body: JSON.stringify({ name }),
    });
  }

  // Get a single show
  async getShow(id) {
    return this.request(`/shows/${id}`);
  }

  // Update a show
  async updateShow(id, name) {
    return this.request(`/shows/${id}`, {
      method: 'PUT',
      body: JSON.stringify({ name }),
    });
  }

  // Delete a show
  async deleteShow(id) {
    return this.request(`/shows/${id}`, {
      method: 'DELETE',
    });
  }

  // Select a show (set as default)
  async selectShow(id) {
    return this.request(`/shows/${id}/select`, {
      method: 'POST',
    });
  }

  // Get statistics
  async getStatistics() {
    return this.request('/statistics');
  }

  // Get drive statistics
  async getDriveStatistics() {
    return this.request('/statistics/drives');
  }

  // Get error frequency statistics
  async getErrorFrequency() {
    return this.request('/statistics/errors');
  }

  // Get rip history
  async getRipHistory(limit = 50) {
    return this.request(`/rip-history?limit=${limit}`);
  }

  // Get user preferences
  async getPreferences() {
    return this.request('/preferences');
  }

  // Update user preferences
  async updatePreferences(prefs) {
    return this.request('/preferences', {
      method: 'POST',
      body: JSON.stringify(prefs),
    });
  }

  // Get drive statistics
  async getDriveStats() {
    return await this.request('GET', '/statistics/drives');
  }

  // Get monitor operations
  async getMonitorOperations() {
    return this.request('/monitor/operations');
  }

  // Get monitor drives
  async getMonitorDrives() {
    return this.request('/monitor/drives');
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
