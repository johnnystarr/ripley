/**
 * Sound notification utility for Ripley
 * Plays sound effects for completed rip operations
 */

/**
 * Play a notification sound
 * @param {string} type - 'success' or 'error'
 */
export function playNotificationSound(type = 'success') {
  try {
    // Create an AudioContext
    const audioContext = new (window.AudioContext || window.webkitAudioContext)();
    
    if (type === 'success') {
      playSuccessSound(audioContext);
    } else {
      playErrorSound(audioContext);
    }
  } catch (error) {
    console.warn('Failed to play notification sound:', error);
  }
}

/**
 * Play a success notification sound (pleasant chime)
 * @param {AudioContext} audioContext
 */
function playSuccessSound(audioContext) {
  const now = audioContext.currentTime;
  
  // Create oscillator for a pleasant two-tone chime
  const oscillator1 = audioContext.createOscillator();
  const oscillator2 = audioContext.createOscillator();
  const gainNode = audioContext.createGain();
  
  oscillator1.connect(gainNode);
  oscillator2.connect(gainNode);
  gainNode.connect(audioContext.destination);
  
  // First note (C5 - 523.25 Hz)
  oscillator1.frequency.setValueAtTime(523.25, now);
  oscillator1.frequency.setValueAtTime(523.25, now + 0.1);
  
  // Second note (E5 - 659.25 Hz)
  oscillator2.frequency.setValueAtTime(659.25, now);
  oscillator2.frequency.setValueAtTime(659.25, now + 0.1);
  
  // Envelope for smooth sound
  gainNode.gain.setValueAtTime(0, now);
  gainNode.gain.linearRampToValueAtTime(0.3, now + 0.01);
  gainNode.gain.exponentialRampToValueAtTime(0.01, now + 0.4);
  
  oscillator1.start(now);
  oscillator2.start(now);
  oscillator1.stop(now + 0.4);
  oscillator2.stop(now + 0.4);
}

/**
 * Play an error notification sound (lower tone)
 * @param {AudioContext} audioContext
 */
function playErrorSound(audioContext) {
  const now = audioContext.currentTime;
  
  const oscillator = audioContext.createOscillator();
  const gainNode = audioContext.createGain();
  
  oscillator.connect(gainNode);
  gainNode.connect(audioContext.destination);
  
  // Lower frequency for error (A3 - 220 Hz)
  oscillator.frequency.setValueAtTime(220, now);
  oscillator.type = 'sine';
  
  // Envelope
  gainNode.gain.setValueAtTime(0, now);
  gainNode.gain.linearRampToValueAtTime(0.3, now + 0.01);
  gainNode.gain.exponentialRampToValueAtTime(0.01, now + 0.3);
  
  oscillator.start(now);
  oscillator.stop(now + 0.3);
}
