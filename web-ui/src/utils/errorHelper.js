/**
 * Error helper utilities for Ripley
 * Provides user-friendly error messages and suggested fixes
 */

export function getErrorCategory(error) {
  const errorLower = error?.toLowerCase() || '';
  
  if (errorLower.includes('drive') || errorLower.includes('device') || errorLower.includes('disc')) {
    return 'drive';
  }
  if (errorLower.includes('network') || errorLower.includes('connection') || errorLower.includes('timeout')) {
    return 'network';
  }
  if (errorLower.includes('permission') || errorLower.includes('access denied')) {
    return 'permission';
  }
  if (errorLower.includes('space') || errorLower.includes('disk full')) {
    return 'storage';
  }
  if (errorLower.includes('api') || errorLower.includes('key') || errorLower.includes('authentication')) {
    return 'api';
  }
  return 'unknown';
}

export function getErrorSuggestion(error) {
  const category = getErrorCategory(error);
  
  const suggestions = {
    drive: {
      title: 'Drive Issue',
      icon: 'faCompactDisc',
      color: 'yellow',
      suggestions: [
        'Eject and re-insert the disc',
        'Clean the disc surface',
        'Try a different optical drive',
        'Check if the disc is scratched or damaged',
      ],
    },
    network: {
      title: 'Network Error',
      icon: 'faNetworkWired',
      color: 'blue',
      suggestions: [
        'Check your internet connection',
        'Verify firewall settings',
        'Try again in a few moments',
        'Check if external services are accessible',
      ],
    },
    permission: {
      title: 'Permission Denied',
      icon: 'faLock',
      color: 'red',
      suggestions: [
        'Check file/folder permissions',
        'Run Ripley with appropriate permissions',
        'Verify output directory is writable',
        'Check disk mount permissions',
      ],
    },
    storage: {
      title: 'Storage Issue',
      icon: 'faHdd',
      color: 'orange',
      suggestions: [
        'Free up disk space',
        'Check available storage on output drive',
        'Choose a different output location',
        'Delete old ripped files if needed',
      ],
    },
    api: {
      title: 'API Configuration',
      icon: 'faKey',
      color: 'purple',
      suggestions: [
        'Verify API keys in Configuration',
        'Test API connection',
        'Check API key has required permissions',
        'Ensure API services are operational',
      ],
    },
    unknown: {
      title: 'Unknown Error',
      icon: 'faExclamationTriangle',
      color: 'red',
      suggestions: [
        'Check the logs for more details',
        'Try the operation again',
        'Verify all settings in Configuration',
        'Contact support if issue persists',
      ],
    },
  };
  
  return suggestions[category];
}

export function formatErrorMessage(error) {
  if (!error) return 'An unknown error occurred';
  
  // Clean up common error prefixes
  let message = error
    .replace(/^Error:\s*/i, '')
    .replace(/^Failed to\s*/i, '')
    .replace(/^Unable to\s*/i, '');
  
  // Capitalize first letter
  message = message.charAt(0).toUpperCase() + message.slice(1);
  
  return message;
}
