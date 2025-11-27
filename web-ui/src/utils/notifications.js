/**
 * Browser notifications utility for Ripley
 * Handles desktop notification permissions and display
 */

let notificationPermission = null;

/**
 * Request browser notification permission
 * @returns {Promise<boolean>} True if granted
 */
export async function requestNotificationPermission() {
  if (!('Notification' in window)) {
    console.warn('Browser does not support notifications');
    return false;
  }

  if (Notification.permission === 'granted') {
    notificationPermission = true;
    return true;
  }

  if (Notification.permission === 'denied') {
    notificationPermission = false;
    return false;
  }

  try {
    const permission = await Notification.requestPermission();
    notificationPermission = permission === 'granted';
    return notificationPermission;
  } catch (error) {
    console.error('Error requesting notification permission:', error);
    return false;
  }
}

/**
 * Check current notification permission status
 * @returns {string} Permission status: 'granted', 'denied', or 'default'
 */
export function getNotificationPermission() {
  if (!('Notification' in window)) {
    return 'denied';
  }
  return Notification.permission;
}

/**
 * Show a desktop notification for a completed rip
 * @param {Object} options Notification options
 * @param {string} options.title The disc title
 * @param {string} options.status 'success' or 'failed'
 * @param {string} [options.message] Additional message
 * @param {Function} [options.onClick] Click handler
 */
export function showRipNotification({ title, status, message, onClick }) {
  if (!('Notification' in window)) {
    return;
  }

  if (Notification.permission !== 'granted') {
    return;
  }

  const isSuccess = status === 'success';
  const body = message || (isSuccess 
    ? 'Disc ripped successfully' 
    : 'Rip operation failed');

  const notification = new Notification(`Ripley: ${title}`, {
    body,
    icon: '/ripley-head.png',
    badge: '/ripley-head.png',
    tag: 'ripley-rip',
    requireInteraction: !isSuccess, // Keep failed notifications visible
    silent: false,
  });

  if (onClick) {
    notification.onclick = () => {
      window.focus();
      onClick();
      notification.close();
    };
  } else {
    notification.onclick = () => {
      window.focus();
      notification.close();
    };
  }

  // Auto-close success notifications after 5 seconds
  if (isSuccess) {
    setTimeout(() => notification.close(), 5000);
  }
}

/**
 * Show a generic notification
 * @param {Object} options Notification options
 * @param {string} options.title Notification title
 * @param {string} options.body Notification body
 * @param {Function} [options.onClick] Click handler
 */
export function showNotification({ title, body, onClick }) {
  if (!('Notification' in window)) {
    return;
  }

  if (Notification.permission !== 'granted') {
    return;
  }

  const notification = new Notification(title, {
    body,
    icon: '/ripley-head.png',
    badge: '/ripley-head.png',
    tag: 'ripley-notification',
  });

  if (onClick) {
    notification.onclick = () => {
      window.focus();
      onClick();
      notification.close();
    };
  }

  setTimeout(() => notification.close(), 5000);
}
