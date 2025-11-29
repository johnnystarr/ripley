/**
 * Sound notification utility for Ripley
 * Plays sound effects for completed rip operations
 * Uses actual sound files from ~/.config/ripley/sounds (copied to public/sounds during build)
 */

// Audio elements are created once and reused
let successAudio = null;
let errorAudio = null;

/**
 * Initialize audio elements (called once)
 */
function initAudio() {
  if (successAudio && errorAudio) {
    return; // Already initialized
  }

  try {
    // Try to load sound files from public/sounds directory
    successAudio = new Audio('/sounds/complete.mp3');
    successAudio.preload = 'auto';
    successAudio.volume = 0.7; // Moderate volume
    
    errorAudio = new Audio('/sounds/error.mp3');
    errorAudio.preload = 'auto';
    errorAudio.volume = 0.7;
    
    // Handle load errors gracefully
    successAudio.addEventListener('error', () => {
      console.warn('Failed to load success sound file, falling back to generated sound');
      successAudio = null;
    });
    
    errorAudio.addEventListener('error', () => {
      console.warn('Failed to load error sound file, falling back to generated sound');
      errorAudio = null;
    });
  } catch (error) {
    console.warn('Failed to initialize audio elements:', error);
  }
}

/**
 * Play a notification sound
 * @param {string} type - 'success' or 'error'
 */
export function playNotificationSound(type = 'success') {
  // Initialize audio on first use
  if (!successAudio || !errorAudio) {
    initAudio();
  }

  try {
    if (type === 'success') {
      if (successAudio) {
        // Reset to beginning and play
        successAudio.currentTime = 0;
        successAudio.play().catch(err => {
          console.warn('Failed to play success sound:', err);
          // Fallback to generated sound
          playSuccessSoundFallback();
        });
      } else {
        // Fallback to generated sound if file doesn't exist
        playSuccessSoundFallback();
      }
    } else {
      if (errorAudio) {
        // Reset to beginning and play
        errorAudio.currentTime = 0;
        errorAudio.play().catch(err => {
          console.warn('Failed to play error sound:', err);
          // Fallback to generated sound
          playErrorSoundFallback();
        });
      } else {
        // Fallback to generated sound if file doesn't exist
        playErrorSoundFallback();
      }
    }
  } catch (error) {
    console.warn('Failed to play notification sound:', error);
    // Fallback to generated sounds
    if (type === 'success') {
      playSuccessSoundFallback();
    } else {
      playErrorSoundFallback();
    }
  }
}

/**
 * Fallback: Play a success notification sound using Web Audio API
 */
function playSuccessSoundFallback() {
  try {
    const audioContext = new (window.AudioContext || window.webkitAudioContext)();
    const now = audioContext.currentTime;
    
    const oscillator1 = audioContext.createOscillator();
    const oscillator2 = audioContext.createOscillator();
    const gainNode = audioContext.createGain();
    
    oscillator1.connect(gainNode);
    oscillator2.connect(gainNode);
    gainNode.connect(audioContext.destination);
    
    oscillator1.frequency.setValueAtTime(523.25, now);
    oscillator2.frequency.setValueAtTime(659.25, now);
    
    gainNode.gain.setValueAtTime(0, now);
    gainNode.gain.linearRampToValueAtTime(0.3, now + 0.01);
    gainNode.gain.exponentialRampToValueAtTime(0.01, now + 0.4);
    
    oscillator1.start(now);
    oscillator2.start(now);
    oscillator1.stop(now + 0.4);
    oscillator2.stop(now + 0.4);
  } catch (error) {
    console.warn('Failed to play fallback success sound:', error);
  }
}

/**
 * Fallback: Play an error notification sound using Web Audio API
 */
function playErrorSoundFallback() {
  try {
    const audioContext = new (window.AudioContext || window.webkitAudioContext)();
    const now = audioContext.currentTime;
    
    const oscillator = audioContext.createOscillator();
    const gainNode = audioContext.createGain();
    
    oscillator.connect(gainNode);
    gainNode.connect(audioContext.destination);
    
    oscillator.frequency.setValueAtTime(220, now);
    oscillator.type = 'sine';
    
    gainNode.gain.setValueAtTime(0, now);
    gainNode.gain.linearRampToValueAtTime(0.3, now + 0.01);
    gainNode.gain.exponentialRampToValueAtTime(0.01, now + 0.3);
    
    oscillator.start(now);
    oscillator.stop(now + 0.3);
  } catch (error) {
    console.warn('Failed to play fallback error sound:', error);
  }
}
