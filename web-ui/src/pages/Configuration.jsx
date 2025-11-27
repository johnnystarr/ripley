import { useState, useEffect } from 'react';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import {
  faSave,
  faSpinner,
  faKey,
  faBell,
  faSync,
  faFilm,
  faMicrophone,
  faEye,
  faEyeSlash,
  faCircleCheck,
  faCircleXmark,
  faCompactDisc,
  faCog,
} from '@fortawesome/free-solid-svg-icons';
import toast from 'react-hot-toast';
import { api } from '../api';
import Dropdown from '../components/Dropdown';
import CollapsibleSection from '../components/CollapsibleSection';
import Tooltip from '../components/Tooltip';

export default function Configuration() {
  const [config, setConfig] = useState(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [showOpenAIKey, setShowOpenAIKey] = useState(false);
  const [showTMDBKey, setShowTMDBKey] = useState(false);
  const [testingOpenAI, setTestingOpenAI] = useState(false);
  const [testingTMDB, setTestingTMDB] = useState(false);
  const [openAIValid, setOpenAIValid] = useState(null);
  const [tmdbValid, setTMDBValid] = useState(null);

  useEffect(() => {
    fetchConfig();
  }, []);

  const fetchConfig = async () => {
    try {
      setLoading(true);
      const data = await api.getConfig();
      setConfig(data);
    } catch (err) {
      toast.error('Failed to load configuration: ' + err.message);
    } finally {
      setLoading(false);
    }
  };

  const handleSave = async () => {
    try {
      setSaving(true);
      await api.updateConfig(config);
      toast.success('Configuration saved successfully!');
    } catch (err) {
      toast.error('Failed to save configuration: ' + err.message);
    } finally {
      setSaving(false);
    }
  };

  const updateConfig = (path, value) => {
    setConfig(prev => {
      const newConfig = { ...prev };
      const keys = path.split('.');
      let current = newConfig;
      
      for (let i = 0; i < keys.length - 1; i++) {
        current = current[keys[i]];
      }
      
      current[keys[keys.length - 1]] = value;
      return newConfig;
    });
    
    // Clear validation when key changes
    if (path === 'openai_api_key') {
      setOpenAIValid(null);
    } else if (path === 'tmdb_api_key') {
      setTMDBValid(null);
    }
  };

  const testOpenAIConnection = async () => {
    if (!config.openai_api_key) {
      toast.error('Please enter an OpenAI API key first');
      return;
    }
    
    setTestingOpenAI(true);
    try {
      const response = await fetch('https://api.openai.com/v1/models', {
        headers: {
          'Authorization': `Bearer ${config.openai_api_key}`,
        },
      });
      
      if (response.ok) {
        setOpenAIValid(true);
        toast.success('OpenAI API key is valid!');
      } else {
        setOpenAIValid(false);
        toast.error('OpenAI API key is invalid');
      }
    } catch (err) {
      setOpenAIValid(false);
      toast.error('Failed to test OpenAI connection: ' + err.message);
    } finally {
      setTestingOpenAI(false);
    }
  };

  const testTMDBConnection = async () => {
    if (!config.tmdb_api_key) {
      toast.error('Please enter a TMDB API key first');
      return;
    }
    
    setTestingTMDB(true);
    try {
      const response = await fetch(
        `https://api.themoviedb.org/3/configuration?api_key=${config.tmdb_api_key}`
      );
      
      if (response.ok) {
        setTMDBValid(true);
        toast.success('TMDB API key is valid!');
      } else {
        setTMDBValid(false);
        toast.error('TMDB API key is invalid');
      }
    } catch (err) {
      setTMDBValid(false);
      toast.error('Failed to test TMDB connection: ' + err.message);
    } finally {
      setTestingTMDB(false);
    }
  };

  if (loading) {
    return (
      <div className="flex items-center justify-center h-64">
        <FontAwesomeIcon icon={faSpinner} className="text-cyan-400 text-4xl animate-spin" />
      </div>
    );
  }

  if (!config) {
    return (
      <div className="bg-red-500/10 border border-red-500 text-red-400 px-4 py-3 rounded-lg">
        Failed to load configuration
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
        <h1 className="text-2xl md:text-3xl font-bold text-slate-100">Configuration</h1>
        <button
          onClick={handleSave}
          disabled={saving}
          className="flex items-center justify-center px-4 py-2 bg-cyan-500 hover:bg-cyan-600 disabled:bg-slate-700 disabled:text-slate-500 text-white rounded-lg transition-colors whitespace-nowrap"
        >
          {saving ? (
            <>
              <FontAwesomeIcon icon={faSpinner} className="mr-2 animate-spin" />
              Saving...
            </>
          ) : (
            <>
              <FontAwesomeIcon icon={faSave} className="mr-2" />
              Save Configuration
            </>
          )}
        </button>
      </div>

      {/* API Keys Section */}
      <CollapsibleSection title="API Keys" icon={faKey} defaultOpen={true}>
        <div className="space-y-4 mt-4">
          {/* OpenAI API Key */}
          <div>
            <label className="block text-slate-400 text-sm mb-2 flex items-center gap-2">
              OpenAI API Key
              <span className="text-slate-500 ml-2">(for speech matching)</span>
              <Tooltip text="Used to transcribe audio tracks for automatic episode title matching via Whisper API" />
            </label>
            <div className="flex gap-2">
              <div className="relative flex-1">
                <input
                  type={showOpenAIKey ? 'text' : 'password'}
                  value={config.openai_api_key || ''}
                  onChange={(e) => updateConfig('openai_api_key', e.target.value || null)}
                  placeholder="sk-..."
                  className={`w-full px-4 py-2 pr-12 bg-slate-900 border rounded-lg text-slate-100 placeholder-slate-500 focus:outline-none ${
                    openAIValid === true
                      ? 'border-green-500'
                      : openAIValid === false
                      ? 'border-red-500'
                      : 'border-slate-700 focus:border-cyan-500'
                  }`}
                />
                <div className="absolute right-3 top-1/2 transform -translate-y-1/2 flex items-center gap-2">
                  {openAIValid === true && (
                    <FontAwesomeIcon icon={faCircleCheck} className="text-green-400" />
                  )}
                  {openAIValid === false && (
                    <FontAwesomeIcon icon={faCircleXmark} className="text-red-400" />
                  )}
                  <button
                    type="button"
                    onClick={() => setShowOpenAIKey(!showOpenAIKey)}
                    className="text-slate-400 hover:text-slate-300"
                  >
                    <FontAwesomeIcon icon={showOpenAIKey ? faEyeSlash : faEye} />
                  </button>
                </div>
              </div>
              <button
                type="button"
                onClick={testOpenAIConnection}
                disabled={testingOpenAI || !config.openai_api_key}
                className="px-4 py-2 bg-slate-700 hover:bg-slate-600 disabled:bg-slate-800 disabled:text-slate-600 text-slate-200 rounded-lg transition-colors whitespace-nowrap"
              >
                {testingOpenAI ? (
                  <FontAwesomeIcon icon={faSpinner} className="animate-spin" />
                ) : (
                  'Test'
                )}
              </button>
            </div>
          </div>

          {/* TMDB API Key */}
          <div>
            <label className="block text-slate-400 text-sm mb-2 flex items-center gap-2">
              TMDB API Key
              <span className="text-slate-500 ml-2">(for movie metadata)</span>
              <Tooltip text="Used to fetch movie and TV show metadata from The Movie Database for accurate file naming" />
            </label>
            <div className="flex gap-2">
              <div className="relative flex-1">
                <input
                  type={showTMDBKey ? 'text' : 'password'}
                  value={config.tmdb_api_key || ''}
                  onChange={(e) => updateConfig('tmdb_api_key', e.target.value || null)}
                  placeholder="Enter TMDB API key"
                  className={`w-full px-4 py-2 pr-12 bg-slate-900 border rounded-lg text-slate-100 placeholder-slate-500 focus:outline-none ${
                    tmdbValid === true
                      ? 'border-green-500'
                      : tmdbValid === false
                      ? 'border-red-500'
                      : 'border-slate-700 focus:border-cyan-500'
                  }`}
                />
                <div className="absolute right-3 top-1/2 transform -translate-y-1/2 flex items-center gap-2">
                  {tmdbValid === true && (
                    <FontAwesomeIcon icon={faCircleCheck} className="text-green-400" />
                  )}
                  {tmdbValid === false && (
                    <FontAwesomeIcon icon={faCircleXmark} className="text-red-400" />
                  )}
                  <button
                    type="button"
                    onClick={() => setShowTMDBKey(!showTMDBKey)}
                    className="text-slate-400 hover:text-slate-300"
                  >
                    <FontAwesomeIcon icon={showTMDBKey ? faEyeSlash : faEye} />
                  </button>
                </div>
              </div>
              <button
                type="button"
                onClick={testTMDBConnection}
                disabled={testingTMDB || !config.tmdb_api_key}
                className="px-4 py-2 bg-slate-700 hover:bg-slate-600 disabled:bg-slate-800 disabled:text-slate-600 text-slate-200 rounded-lg transition-colors whitespace-nowrap"
              >
                {testingTMDB ? (
                  <FontAwesomeIcon icon={faSpinner} className="animate-spin" />
                ) : (
                  'Test'
                )}
              </button>
            </div>
          </div>
        </div>
      </CollapsibleSection>

      {/* Notifications Section */}
      <CollapsibleSection title="Notifications" icon={faBell} defaultOpen={false}>
        <div className="space-y-4 mt-4">
          <div className="flex items-center justify-between">
            <div>
              <div className="flex items-center gap-2">
                <label className="text-slate-100 font-medium">Enable Notifications</label>
                <Tooltip text="Send system notifications when rip operations complete or fail" />
              </div>
              <p className="text-slate-400 text-sm mt-1">Send push notifications for rip events</p>
            </div>
            <button
              onClick={() => updateConfig('notifications.enabled', !config.notifications.enabled)}
              className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors ${
                config.notifications.enabled ? 'bg-cyan-500' : 'bg-slate-700'
              }`}
            >
              <span
                className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform ${
                  config.notifications.enabled ? 'translate-x-6' : 'translate-x-1'
                }`}
              />
            </button>
          </div>

          <div>
            <label className="block text-slate-400 text-sm mb-2 flex items-center gap-2">
              Notification Topic
              <Tooltip text="Topic name for push notification service routing" />
            </label>
            <input
              type="text"
              value={config.notifications.topic}
              onChange={(e) => updateConfig('notifications.topic', e.target.value)}
              placeholder="notification_topic"
              className="w-full px-4 py-2 bg-slate-900 border border-slate-700 rounded-lg text-slate-100 placeholder-slate-500 focus:outline-none focus:border-cyan-500"
            />
          </div>
        </div>
      </CollapsibleSection>

      {/* Rsync Section */}
      <CollapsibleSection title="Rsync Backup" icon={faSync} defaultOpen={false}>
        <div className="space-y-4 mt-4">
          <div className="flex items-center justify-between">
            <div>
              <div className="flex items-center gap-2">
                <label className="text-slate-100 font-medium">Enable Rsync Backup</label>
                <Tooltip text="Automatically backup ripped files to a remote destination using rsync" />
              </div>
              <p className="text-slate-400 text-sm mt-1">Automatically backup ripped files</p>
            </div>
            <button
              onClick={() => updateConfig('rsync.enabled', !config.rsync.enabled)}
              className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors ${
                config.rsync.enabled ? 'bg-cyan-500' : 'bg-slate-700'
              }`}
            >
              <span
                className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform ${
                  config.rsync.enabled ? 'translate-x-6' : 'translate-x-1'
                }`}
              />
            </button>
          </div>

          <div>
            <label className="block text-slate-400 text-sm mb-2 flex items-center gap-2">
              Rsync Destination
              <Tooltip text="Destination path for rsync backup (e.g., user@host:/path/to/backup)" />
            </label>
            <input
              type="text"
              value={config.rsync.destination}
              onChange={(e) => updateConfig('rsync.destination', e.target.value)}
              placeholder="/path/to/backup"
              className="w-full px-4 py-2 bg-slate-900 border border-slate-700 rounded-lg text-slate-100 placeholder-slate-500 focus:outline-none focus:border-cyan-500"
            />
          </div>
        </div>
      </CollapsibleSection>

      {/* Speech Match Section */}
      <CollapsibleSection title="Speech Matching" icon={faMicrophone} defaultOpen={false}>
        <div className="space-y-4 mt-4">
          <div className="flex items-center justify-between">
            <div>
              <label className="text-slate-100 font-medium">Enable Speech Match</label>
              <p className="text-slate-400 text-sm mt-1">Use Whisper to identify episode titles</p>
            </div>
            <button
              onClick={() => updateConfig('speech_match.enabled', !config.speech_match.enabled)}
              className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors ${
                config.speech_match.enabled ? 'bg-cyan-500' : 'bg-slate-700'
              }`}
            >
              <span
                className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform ${
                  config.speech_match.enabled ? 'translate-x-6' : 'translate-x-1'
                }`}
              />
            </button>
          </div>

          <div>
            <label className="block text-slate-400 text-sm mb-2">
              Audio Duration (seconds)
              <span className="text-slate-500 ml-2">{config.speech_match.audio_duration}s</span>
            </label>
            <input
              type="range"
              min="30"
              max="600"
              value={config.speech_match.audio_duration}
              onChange={(e) => updateConfig('speech_match.audio_duration', parseInt(e.target.value))}
              className="w-full h-2 bg-slate-700 rounded-lg appearance-none cursor-pointer accent-cyan-500"
            />
            <div className="flex justify-between text-xs text-slate-500 mt-1">
              <span>30s</span>
              <span>10min</span>
            </div>
          </div>

          <Dropdown
            label="Whisper Model"
            value={config.speech_match.whisper_model}
            onChange={(value) => updateConfig('speech_match.whisper_model', value)}
            options={[
              { value: 'tiny', label: 'Tiny (fastest, least accurate)' },
              { value: 'base', label: 'Base (recommended)' },
              { value: 'small', label: 'Small (slower, more accurate)' },
              { value: 'medium', label: 'Medium (very slow)' },
              { value: 'large', label: 'Large (slowest, best accuracy)' },
            ]}
          />

          <div className="flex items-center justify-between">
            <div>
              <label className="text-slate-100 font-medium">Use OpenAI API</label>
              <p className="text-slate-400 text-sm mt-1">Use cloud API instead of local Whisper</p>
            </div>
            <button
              onClick={() => updateConfig('speech_match.use_openai_api', !config.speech_match.use_openai_api)}
              className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors ${
                config.speech_match.use_openai_api ? 'bg-cyan-500' : 'bg-slate-700'
              }`}
            >
              <span
                className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform ${
                  config.speech_match.use_openai_api ? 'translate-x-6' : 'translate-x-1'
                }`}
              />
            </button>
          </div>
        </div>
      </CollapsibleSection>

      {/* Filebot Section */}
      <CollapsibleSection title="Filebot Metadata" icon={faFilm} defaultOpen={false}>
        <div className="space-y-4 mt-4">
          <div className="flex items-center justify-between">
            <div>
              <label className="text-slate-100 font-medium">Skip by Default</label>
              <p className="text-slate-400 text-sm mt-1">Skip Filebot processing unless specified</p>
            </div>
            <button
              onClick={() => updateConfig('filebot.skip_by_default', !config.filebot.skip_by_default)}
              className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors ${
                config.filebot.skip_by_default ? 'bg-cyan-500' : 'bg-slate-700'
              }`}
            >
              <span
                className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform ${
                  config.filebot.skip_by_default ? 'translate-x-6' : 'translate-x-1'
                }`}
              />
            </button>
          </div>

          <Dropdown
            label="Database"
            value={config.filebot.database}
            onChange={(value) => updateConfig('filebot.database', value)}
            options={[
              { value: 'TheTVDB', label: 'TheTVDB' },
              { value: 'TheMovieDB', label: 'TheMovieDB' },
            ]}
          />

          <Dropdown
            label="Episode Order"
            value={config.filebot.order}
            onChange={(value) => updateConfig('filebot.order', value)}
            options={[
              { value: 'Airdate', label: 'Airdate' },
              { value: 'DVD', label: 'DVD' },
            ]}
          />

          <div className="flex items-center justify-between">
            <div>
              <label className="text-slate-100 font-medium">Use for Music</label>
              <p className="text-slate-400 text-sm mt-1">Standardize music file names with Filebot</p>
            </div>
            <button
              onClick={() => updateConfig('filebot.use_for_music', !config.filebot.use_for_music)}
              className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors ${
                config.filebot.use_for_music ? 'bg-cyan-500' : 'bg-slate-700'
              }`}
            >
              <span
                className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform ${
                  config.filebot.use_for_music ? 'translate-x-6' : 'translate-x-1'
                }`}
              />
            </button>
          </div>
        </div>
      </CollapsibleSection>

      {/* Save Button */}
      <div className="flex justify-end">
        <button
          onClick={handleSave}
          disabled={saving}
          className="flex items-center px-6 py-3 bg-cyan-500 hover:bg-cyan-600 disabled:bg-slate-700 disabled:text-slate-500 text-white rounded-lg transition-colors text-lg"
        >
          {saving ? (
            <>
              <FontAwesomeIcon icon={faSpinner} className="mr-2 animate-spin" />
              Saving...
            </>
          ) : (
            <>
              <FontAwesomeIcon icon={faSave} className="mr-2" />
              Save Configuration
            </>
          )}
        </button>
      </div>
    </div>
  );
}
