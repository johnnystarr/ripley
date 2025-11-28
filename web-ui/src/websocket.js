// WebSocket connection manager for real-time updates

import { api } from './api';

export class WebSocketManager {
  constructor() {
    this.ws = null;
    this.listeners = new Map();
    this.reconnectAttempts = 0;
    this.maxReconnectAttempts = 10;
    this.reconnectDelay = 1000;
    this.isIntentionallyClosed = false;
  }

  connect() {
    // If already connected, don't reconnect
    if (this.ws && this.ws.readyState === WebSocket.OPEN) {
      return;
    }

    // If there's a connection in progress (CONNECTING), wait for it
    if (this.ws && this.ws.readyState === WebSocket.CONNECTING) {
      console.log('WebSocket connection already in progress, waiting...');
      return;
    }

    // Clean up any existing connection that's closing or closed
    if (this.ws) {
      if (this.ws.readyState === WebSocket.CLOSING || this.ws.readyState === WebSocket.CLOSED) {
        this.ws = null;
      } else {
        // If it's in an unexpected state, close it first
        try {
          this.ws.close();
        } catch (e) {
          // Ignore errors when closing
        }
        this.ws = null;
      }
    }

    this.isIntentionallyClosed = false;
    const url = api.getWebSocketUrl();
    console.log('Connecting to WebSocket:', url);

    try {
      this.ws = new WebSocket(url);

      this.ws.onopen = () => {
        console.log('WebSocket connected');
        this.reconnectAttempts = 0;
        this.emit('connection', { connected: true });
      };

      this.ws.onmessage = (event) => {
        try {
          const message = JSON.parse(event.data);
          console.log('WebSocket message:', message);
          
          // Emit event based on type
          if (message.type) {
            this.emit(message.type, message.data);
          }
        } catch (error) {
          console.error('Failed to parse WebSocket message:', error);
        }
      };

      this.ws.onerror = (error) => {
        console.error('WebSocket error:', error);
        // Don't emit error if connection was intentionally closed
        if (!this.isIntentionallyClosed) {
          this.emit('error', error);
        }
      };

      this.ws.onclose = (event) => {
        const wasOpen = event.wasClean === true || (event.code === 1000 || event.code === 1001);
        const wasConnecting = event.code === 1006; // Abnormal closure often means connection never established
        
        console.log('WebSocket disconnected', {
          code: event.code,
          reason: event.reason || 'No reason provided',
          wasClean: event.wasClean,
          wasOpen,
          wasConnecting
        });
        
        // Store reference before cleanup
        const wsRef = this.ws;
        
        // Clean up the WebSocket reference
        this.ws = null;
        
        // Only emit connection event if it was actually connected (not just failed to connect)
        if (wasOpen && !wasConnecting) {
          this.emit('connection', { connected: false });
        } else if (wasConnecting) {
          // Connection failed before establishing - this is expected on first attempt sometimes
          console.log('WebSocket connection failed before establishing - will retry');
        }
        
        // Attempt to reconnect if not intentionally closed
        if (!this.isIntentionallyClosed && this.reconnectAttempts < this.maxReconnectAttempts) {
          this.reconnectAttempts++;
          const delay = this.reconnectDelay * Math.pow(2, this.reconnectAttempts - 1);
          console.log(`Reconnecting in ${delay}ms (attempt ${this.reconnectAttempts}/${this.maxReconnectAttempts})`);
          
          setTimeout(() => {
            // Only reconnect if we're still supposed to be connected
            if (!this.isIntentionallyClosed) {
              this.connect();
            }
          }, delay);
        }
      };
    } catch (error) {
      console.error('Failed to create WebSocket:', error);
    }
  }

  disconnect() {
    this.isIntentionallyClosed = true;
    if (this.ws) {
      // Only close if not already closed or closing
      if (this.ws.readyState === WebSocket.OPEN || this.ws.readyState === WebSocket.CONNECTING) {
        try {
          this.ws.close(1000, 'Client disconnecting'); // Normal closure
        } catch (e) {
          console.error('Error closing WebSocket:', e);
        }
      }
      this.ws = null;
    }
  }

  on(eventType, callback) {
    if (!this.listeners.has(eventType)) {
      this.listeners.set(eventType, []);
    }
    this.listeners.get(eventType).push(callback);

    // Return unsubscribe function
    return () => {
      const callbacks = this.listeners.get(eventType);
      if (callbacks) {
        const index = callbacks.indexOf(callback);
        if (index > -1) {
          callbacks.splice(index, 1);
        }
      }
    };
  }

  off(eventType, callback) {
    const callbacks = this.listeners.get(eventType);
    if (callbacks) {
      const index = callbacks.indexOf(callback);
      if (index > -1) {
        callbacks.splice(index, 1);
      }
    }
  }

  emit(eventType, data) {
    const callbacks = this.listeners.get(eventType);
    if (callbacks) {
      callbacks.forEach(callback => {
        try {
          callback(data);
        } catch (error) {
          console.error(`Error in ${eventType} listener:`, error);
        }
      });
    }
  }

  isConnected() {
    return this.ws && this.ws.readyState === WebSocket.OPEN;
  }
}

// Global WebSocket instance
export const wsManager = new WebSocketManager();
