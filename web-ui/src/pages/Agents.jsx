import { useState, useEffect, useCallback, useMemo } from 'react';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import {
  faDesktop,
  faSpinner,
  faCircle,
  faCircleCheck,
  faCircleXmark,
  faClock,
  faCog,
  faTrash,
  faEdit,
  faPlus,
  faSave,
  faTimes,
  faChartLine,
  faFilm,
  faList,
  faBan,
  faCheck,
  faChevronDown,
  faChevronUp,
  faPowerOff,
  faHistory,
} from '@fortawesome/free-solid-svg-icons';
import toast from 'react-hot-toast';
import { api } from '../api';
import { wsManager } from '../websocket';
import Dropdown from '../components/Dropdown';

function formatRelativeTime(dateString) {
  if (!dateString) return 'Never';
  
  const date = new Date(dateString);
  const now = new Date();
  const diffMs = now - date;
  const diffSecs = Math.floor(diffMs / 1000);
  const diffMins = Math.floor(diffSecs / 60);
  const diffHours = Math.floor(diffMins / 60);
  
  if (diffSecs < 60) return 'Just now';
  if (diffMins < 60) return `${diffMins}m ago`;
  if (diffHours < 24) return `${diffHours}h ago`;
  return `${Math.floor(diffHours / 24)}d ago`;
}

export default function Agents() {
  const [agents, setAgents] = useState([]);
  const [profiles, setProfiles] = useState([]);
  const [jobs, setJobs] = useState([]);
  const [loading, setLoading] = useState(true);
  const [activeTab, setActiveTab] = useState('agents'); // 'agents', 'profiles', 'jobs'
  const [expandedAgent, setExpandedAgent] = useState(null);
  const [expandedProfile, setExpandedProfile] = useState(null);
  const [isCreatingProfile, setIsCreatingProfile] = useState(false);
  const [editingProfile, setEditingProfile] = useState(null);
  const [editingProfileData, setEditingProfileData] = useState({ name: '', description: '', settings_json: {} });
  const [newProfile, setNewProfile] = useState({ name: '', description: '', settings_json: {} });
  const [shows, setShows] = useState([]);
  const [profileShowAssociations, setProfileShowAssociations] = useState({}); // profile_id -> [show_ids]
  const [addingShowToProfile, setAddingShowToProfile] = useState(null);
  const [editingOutputLocation, setEditingOutputLocation] = useState(null);
  const [outputLocationValue, setOutputLocationValue] = useState('');

  // Fetch data on mount
  useEffect(() => {
    fetchAgents();
    fetchProfiles();
    fetchJobs();
    fetchShows();
    
    // Poll for updates every 3 seconds
    const interval = setInterval(() => {
      fetchAgents();
      fetchJobs();
    }, 3000);

    return () => clearInterval(interval);
  }, []);

  // Set up WebSocket listeners for real-time agent updates
  useEffect(() => {
    const unsubscribeAgentStatus = wsManager.on('AgentStatusChanged', (data) => {
      setAgents(prev => prev.map(agent => 
        agent.agent_id === data.agent_id
          ? { ...agent, status: data.status, last_seen: data.last_seen }
          : agent
      ));
    });

    const unsubscribeJobStatus = wsManager.on('UpscalingJobStatusChanged', (data) => {
      setJobs(prev => prev.map(job => 
        job.job_id === data.job_id
          ? { ...job, status: data.status, progress: data.progress, error_message: data.error_message }
          : job
      ));
    });

    return () => {
      unsubscribeAgentStatus();
      unsubscribeJobStatus();
    };
  }, []);

  const fetchAgents = useCallback(async () => {
    try {
      const data = await api.getAgents();
      setAgents(data);
    } catch (err) {
      console.error('Failed to fetch agents:', err);
      toast.error('Failed to load agents');
    } finally {
      setLoading(false);
    }
  }, []);

  const fetchProfiles = useCallback(async () => {
    try {
      const data = await api.getTopazProfiles();
      setProfiles(data);
      // Fetch show associations - iterate through all shows to find associations
      const associations = {};
      const allShows = await api.getShows();
      for (const show of allShows) {
        if (show.id) {
          try {
            const showProfiles = await api.getProfilesForShow(show.id);
            for (const profile of showProfiles) {
              if (profile.id) {
                if (!associations[profile.id]) {
                  associations[profile.id] = [];
                }
                associations[profile.id].push(show.id);
              }
            }
          } catch (err) {
            // Show might not have any profiles, which is fine
          }
        }
      }
      setProfileShowAssociations(associations);
    } catch (err) {
      console.error('Failed to fetch profiles:', err);
      toast.error('Failed to load Topaz profiles');
    }
  }, []);

  const fetchJobs = useCallback(async () => {
    try {
      const data = await api.getUpscalingJobs();
      setJobs(data);
    } catch (err) {
      console.error('Failed to fetch jobs:', err);
    }
  }, []);

  const fetchShows = useCallback(async () => {
    try {
      const data = await api.getShows();
      setShows(data);
    } catch (err) {
      console.error('Failed to fetch shows:', err);
    }
  }, []);

  const getStatusColor = (status) => {
    switch (status) {
      case 'online': return 'text-green-400 bg-green-500/10 border-green-500/30';
      case 'offline': return 'text-slate-400 bg-slate-700/50 border-slate-600/30';
      case 'busy': return 'text-yellow-400 bg-yellow-500/10 border-yellow-500/30';
      default: return 'text-slate-400 bg-slate-700/50 border-slate-600/30';
    }
  };

  const getStatusIcon = (status) => {
    switch (status) {
      case 'online': return faCircleCheck;
      case 'offline': return faCircleXmark;
      case 'busy': return faSpinner;
      default: return faCircle;
    }
  };

  const handleCreateProfile = useCallback(async () => {
    if (!newProfile.name.trim()) {
      toast.error('Profile name is required');
      return;
    }

    try {
      await api.createTopazProfile(newProfile);
      toast.success('Profile created');
      setIsCreatingProfile(false);
      setNewProfile({ name: '', description: '', settings_json: {} });
      fetchProfiles();
    } catch (err) {
      toast.error('Failed to create profile: ' + err.message);
    }
  }, [newProfile, fetchProfiles]);

  const handleDeleteProfile = useCallback(async (id) => {
    if (!window.confirm('Are you sure you want to delete this profile?')) {
      return;
    }

    try {
      await api.deleteTopazProfile(id);
      toast.success('Profile deleted');
      fetchProfiles();
    } catch (err) {
      toast.error('Failed to delete profile: ' + err.message);
    }
  }, [fetchProfiles]);

  const handleEditProfile = useCallback((profile) => {
    setEditingProfile(profile.id);
    setEditingProfileData({
      name: profile.name,
      description: profile.description || '',
      settings_json: profile.settings_json,
    });
  }, []);

  const handleSaveProfile = useCallback(async (id) => {
    try {
      await api.updateTopazProfile(id, {
        name: editingProfileData.name,
        description: editingProfileData.description || null,
        settings_json: editingProfileData.settings_json,
      });
      toast.success('Profile updated');
      setEditingProfile(null);
      fetchProfiles();
    } catch (err) {
      toast.error('Failed to update profile: ' + err.message);
    }
  }, [editingProfileData, fetchProfiles]);

  const handleAssociateShow = useCallback(async (profileId, showId) => {
    try {
      await api.associateProfileWithShow(profileId, showId);
      toast.success('Profile associated with show');
      fetchProfiles();
    } catch (err) {
      toast.error('Failed to associate profile: ' + err.message);
    }
  }, [fetchProfiles]);

  const handleRemoveShowAssociation = useCallback(async (profileId, showId) => {
    try {
      await api.removeProfileFromShow(profileId, showId);
      toast.success('Association removed');
      fetchProfiles();
    } catch (err) {
      toast.error('Failed to remove association: ' + err.message);
    }
  }, [fetchProfiles]);

  const handleEditOutputLocation = useCallback(async (agentId, currentLocation) => {
    setEditingOutputLocation(agentId);
    setOutputLocationValue(currentLocation || '');
  }, []);

  const handleSaveOutputLocation = useCallback(async (agentId) => {
    try {
      await api.updateAgentOutputLocation(agentId, outputLocationValue);
      toast.success('Output location updated');
      setEditingOutputLocation(null);
      setOutputLocationValue('');
      fetchAgents();
    } catch (err) {
      toast.error('Failed to update output location: ' + err.message);
    }
  }, [outputLocationValue, fetchAgents]);

  const handleCancelEditOutputLocation = useCallback(() => {
    setEditingOutputLocation(null);
    setOutputLocationValue('');
  }, []);

  const handleDisconnectAgent = useCallback(async (agentId) => {
    if (!window.confirm('Are you sure you want to disconnect this agent? This will mark it as offline.')) {
      return;
    }

    try {
      await api.disconnectAgent(agentId);
      toast.success('Agent disconnected');
      fetchAgents();
    } catch (err) {
      toast.error('Failed to disconnect agent: ' + err.message);
    }
  }, [fetchAgents]);

  const activeJobs = jobs.filter(job => job.status === 'processing' || job.status === 'assigned');
  const onlineAgents = agents.filter(agent => agent.status === 'online');
  const busyAgents = agents.filter(agent => agent.status === 'busy');

  if (loading && agents.length === 0) {
    return (
      <div className="flex items-center justify-center h-64">
        <FontAwesomeIcon icon={faSpinner} className="text-cyan-400 text-4xl animate-spin" />
      </div>
    );
  }

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
        <div>
          <h1 className="text-2xl md:text-3xl font-bold text-slate-100">GUI Agents</h1>
          <p className="text-slate-400 mt-2 text-sm sm:text-base">
            Manage Windows GUI agents and Topaz Video AI profiles
          </p>
        </div>
        <div className="flex items-center gap-4 text-sm">
          <div className="flex items-center gap-2">
            <span className="text-slate-400">Agents Online:</span>
            <span className="text-green-400 font-medium">{onlineAgents.length}</span>
          </div>
          <div className="flex items-center gap-2">
            <span className="text-slate-400">Active Jobs:</span>
            <span className="text-cyan-400 font-medium">{activeJobs.length}</span>
          </div>
        </div>
      </div>

      {/* Tabs */}
      <div className="border-b border-slate-700">
        <nav className="flex space-x-8">
          {[
            { id: 'agents', label: 'Agents', icon: faDesktop },
            { id: 'profiles', label: 'Topaz Profiles', icon: faFilm },
            { id: 'jobs', label: 'Upscaling Jobs', icon: faList },
          ].map((tab) => (
            <button
              key={tab.id}
              onClick={() => setActiveTab(tab.id)}
              className={`flex items-center gap-2 py-4 px-1 border-b-2 font-medium text-sm transition-colors ${
                activeTab === tab.id
                  ? 'border-cyan-500 text-cyan-400'
                  : 'border-transparent text-slate-400 hover:text-slate-300 hover:border-slate-600'
              }`}
            >
              <FontAwesomeIcon icon={tab.icon} />
              {tab.label}
            </button>
          ))}
        </nav>
      </div>

      {/* Agents Tab */}
      {activeTab === 'agents' && (
        <div className="space-y-4">
          {agents.length === 0 ? (
            <div className="bg-slate-800 rounded-lg p-12 border border-slate-700 text-center">
              <FontAwesomeIcon icon={faDesktop} className="text-slate-600 text-5xl mb-4" />
              <h3 className="text-xl font-semibold text-slate-100 mb-2">No Agents Connected</h3>
              <p className="text-slate-400">Start a Windows GUI agent to connect</p>
            </div>
          ) : (
            agents.map((agent) => {
              const isExpanded = expandedAgent === agent.agent_id;
              const allAgentJobs = jobs.filter(job => job.agent_id === agent.agent_id);
              const activeJobs = allAgentJobs.filter(job => job.status === 'processing' || job.status === 'assigned');
              const queuedJobs = allAgentJobs.filter(job => job.status === 'queued');
              const completedJobs = allAgentJobs.filter(job => job.status === 'completed').slice(0, 10);
              const failedJobs = allAgentJobs.filter(job => job.status === 'failed').slice(0, 10);
              
              return (
                <div
                  key={agent.agent_id}
                  className={`bg-slate-800 rounded-lg border transition-colors ${getStatusColor(agent.status)}`}
                >
                  <div className="p-4">
                    <div className="flex items-start justify-between mb-3">
                      <div className="flex-1">
                        <div className="flex items-center gap-3 mb-1">
                          <FontAwesomeIcon
                            icon={getStatusIcon(agent.status)}
                            className={`text-sm ${
                              agent.status === 'online' ? 'animate-pulse' : ''
                            }`}
                          />
                          <h3 className="font-semibold text-lg">{agent.name}</h3>
                          <span className="px-2 py-1 text-xs rounded bg-slate-900/50">
                            {agent.status}
                          </span>
                          {agent.platform && (
                            <span className="px-2 py-1 text-xs rounded bg-slate-900/50">
                              {agent.platform}
                            </span>
                          )}
                        </div>
                        <div className="text-slate-300 text-sm space-y-1">
                          <div>Agent ID: <span className="text-slate-400 font-mono text-xs">{agent.agent_id}</span></div>
                          {agent.ip_address && (
                            <div>IP: <span className="text-slate-400">{agent.ip_address}</span></div>
                          )}
                          {agent.topaz_version && (
                            <div>Topaz Video AI: <span className="text-cyan-400">{agent.topaz_version}</span></div>
                          )}
                          <div className="flex items-center gap-2 mt-2">
                            <span>Output Location:</span>
                            {editingOutputLocation === agent.agent_id ? (
                              <div className="flex items-center gap-2 flex-1 max-w-md">
                                <input
                                  type="text"
                                  value={outputLocationValue}
                                  onChange={(e) => setOutputLocationValue(e.target.value)}
                                  placeholder="e.g., /path/to/output or C:\Videos\Ripley"
                                  className="flex-1 px-2 py-1 text-xs bg-slate-900 border border-slate-600 rounded text-slate-100 focus:outline-none focus:border-cyan-500"
                                />
                                <button
                                  onClick={() => handleSaveOutputLocation(agent.agent_id)}
                                  className="px-2 py-1 text-xs bg-cyan-600 hover:bg-cyan-700 text-white rounded"
                                  title="Save"
                                >
                                  <FontAwesomeIcon icon={faSave} />
                                </button>
                                <button
                                  onClick={handleCancelEditOutputLocation}
                                  className="px-2 py-1 text-xs bg-slate-700 hover:bg-slate-600 text-slate-300 rounded"
                                  title="Cancel"
                                >
                                  <FontAwesomeIcon icon={faTimes} />
                                </button>
                              </div>
                            ) : (
                              <>
                                <span className="text-slate-400 font-mono text-xs">
                                  {agent.output_location || 'Not set (default: ~/ripley_output)'}
                                </span>
                                <button
                                  onClick={() => handleEditOutputLocation(agent.agent_id, agent.output_location)}
                                  className="ml-2 text-cyan-400 hover:text-cyan-300 text-xs"
                                  title="Edit output location"
                                >
                                  <FontAwesomeIcon icon={faEdit} />
                                </button>
                              </>
                            )}
                          </div>
                        </div>
                      </div>
                      <div className="flex items-center gap-2">
                        <span className="text-xs text-slate-400">
                          <FontAwesomeIcon icon={faClock} className="mr-1" />
                          {formatRelativeTime(agent.last_seen)}
                        </span>
                        {agent.status === 'online' && (
                          <button
                            onClick={() => handleDisconnectAgent(agent.agent_id)}
                            className="px-2 py-1 text-xs bg-red-600/20 hover:bg-red-600/30 text-red-400 rounded transition-colors"
                            title="Disconnect agent"
                          >
                            <FontAwesomeIcon icon={faPowerOff} />
                          </button>
                        )}
                        <button
                          onClick={() => setExpandedAgent(isExpanded ? null : agent.agent_id)}
                          className="text-slate-400 hover:text-slate-300 transition-colors"
                        >
                          <FontAwesomeIcon icon={isExpanded ? faChevronUp : faChevronDown} />
                        </button>
                      </div>
                    </div>

                    {isExpanded && (
                      <div className="mt-4 pt-4 border-t border-slate-700 space-y-4">
                        {/* Active Jobs */}
                        <div>
                          <h4 className="text-sm font-semibold text-slate-300 mb-2">Active Jobs</h4>
                          {activeJobs.length === 0 ? (
                            <p className="text-slate-500 text-sm">No active jobs</p>
                          ) : (
                            <div className="space-y-2">
                              {activeJobs.map((job) => (
                                <div key={job.job_id} className="bg-slate-900/50 rounded p-2 text-xs">
                                  <div className="flex justify-between mb-1">
                                    <span className="text-slate-300 font-mono text-xs">{job.job_id.substring(0, 8)}...</span>
                                    <span className="text-cyan-400 font-medium">{Math.round(job.progress)}%</span>
                                  </div>
                                  <div className="text-slate-400 text-xs mb-1 truncate">{job.input_file_path}</div>
                                  <div className="w-full bg-slate-800 rounded-full h-1.5">
                                    <div
                                      className="bg-cyan-500 h-1.5 rounded-full transition-all"
                                      style={{ width: `${Math.min(job.progress, 100)}%` }}
                                    />
                                  </div>
                                </div>
                              ))}
                            </div>
                          )}
                        </div>

                        {/* Queued Jobs */}
                        {queuedJobs.length > 0 && (
                          <div>
                            <h4 className="text-sm font-semibold text-slate-300 mb-2">Queued Jobs</h4>
                            <div className="space-y-2">
                              {queuedJobs.map((job) => (
                                <div key={job.job_id} className="bg-slate-900/50 rounded p-2 text-xs">
                                  <div className="flex justify-between mb-1">
                                    <span className="text-slate-300 font-mono text-xs">{job.job_id.substring(0, 8)}...</span>
                                    <span className="text-yellow-400 text-xs">Queued</span>
                                  </div>
                                  <div className="text-slate-400 text-xs truncate">{job.input_file_path}</div>
                                </div>
                              ))}
                            </div>
                          </div>
                        )}

                        {/* Job History */}
                        {(completedJobs.length > 0 || failedJobs.length > 0) && (
                          <div>
                            <h4 className="text-sm font-semibold text-slate-300 mb-2 flex items-center gap-2">
                              <FontAwesomeIcon icon={faHistory} />
                              Recent Job History
                            </h4>
                            <div className="space-y-2 max-h-48 overflow-y-auto">
                              {completedJobs.map((job) => (
                                <div key={job.job_id} className="bg-slate-900/50 rounded p-2 text-xs">
                                  <div className="flex justify-between mb-1">
                                    <span className="text-slate-300 font-mono text-xs">{job.job_id.substring(0, 8)}...</span>
                                    <span className="text-green-400 text-xs">Completed</span>
                                  </div>
                                  <div className="text-slate-400 text-xs truncate">{job.input_file_path}</div>
                                  {job.completed_at && (
                                    <div className="text-slate-500 text-xs mt-1">
                                      {formatRelativeTime(job.completed_at)}
                                    </div>
                                  )}
                                </div>
                              ))}
                              {failedJobs.map((job) => (
                                <div key={job.job_id} className="bg-slate-900/50 rounded p-2 text-xs">
                                  <div className="flex justify-between mb-1">
                                    <span className="text-slate-300 font-mono text-xs">{job.job_id.substring(0, 8)}...</span>
                                    <span className="text-red-400 text-xs">Failed</span>
                                  </div>
                                  <div className="text-slate-400 text-xs truncate">{job.input_file_path}</div>
                                  {job.error_message && (
                                    <div className="text-red-400 text-xs mt-1 truncate" title={job.error_message}>
                                      {job.error_message}
                                    </div>
                                  )}
                                  {job.completed_at && (
                                    <div className="text-slate-500 text-xs mt-1">
                                      {formatRelativeTime(job.completed_at)}
                                    </div>
                                  )}
                                </div>
                              ))}
                            </div>
                          </div>
                        )}
                      </div>
                    )}
                  </div>
                </div>
              );
            })
          )}
        </div>
      )}

      {/* Profiles Tab */}
      {activeTab === 'profiles' && (
        <div className="space-y-4">
          <div className="flex justify-between items-center">
            <h2 className="text-xl font-semibold text-slate-100">Topaz Video AI Profiles</h2>
            <button
              onClick={() => setIsCreatingProfile(true)}
              className="px-4 py-2 bg-cyan-500 hover:bg-cyan-600 text-white rounded-lg transition-colors font-medium"
            >
              <FontAwesomeIcon icon={faPlus} className="mr-2" />
              New Profile
            </button>
          </div>

          {isCreatingProfile && (
            <div className="bg-slate-800 rounded-lg p-5 border border-slate-700">
              <h3 className="text-lg font-semibold text-slate-100 mb-4">Create New Profile</h3>
              <div className="space-y-4">
                <div>
                  <label className="block text-sm font-medium text-slate-300 mb-2">Name *</label>
                  <input
                    type="text"
                    value={newProfile.name}
                    onChange={(e) => setNewProfile({ ...newProfile, name: e.target.value })}
                    className="w-full px-4 py-2 bg-slate-900 border border-slate-700 rounded-lg text-slate-100 focus:outline-none focus:border-cyan-500"
                    placeholder="e.g., Standard HD Upscale"
                  />
                </div>
                <div>
                  <label className="block text-sm font-medium text-slate-300 mb-2">Description</label>
                  <textarea
                    value={newProfile.description}
                    onChange={(e) => setNewProfile({ ...newProfile, description: e.target.value })}
                    className="w-full px-4 py-2 bg-slate-900 border border-slate-700 rounded-lg text-slate-100 focus:outline-none focus:border-cyan-500"
                    rows="3"
                    placeholder="Profile description..."
                  />
                </div>
                <div className="flex gap-2">
                  <button
                    onClick={handleCreateProfile}
                    className="px-4 py-2 bg-cyan-500 hover:bg-cyan-600 text-white rounded-lg transition-colors"
                  >
                    <FontAwesomeIcon icon={faSave} className="mr-2" />
                    Create
                  </button>
                  <button
                    onClick={() => {
                      setIsCreatingProfile(false);
                      setNewProfile({ name: '', description: '', settings_json: {} });
                    }}
                    className="px-4 py-2 bg-slate-700 hover:bg-slate-600 text-white rounded-lg transition-colors"
                  >
                    <FontAwesomeIcon icon={faTimes} className="mr-2" />
                    Cancel
                  </button>
                </div>
              </div>
            </div>
          )}

          {profiles.length === 0 ? (
            <div className="bg-slate-800 rounded-lg p-12 border border-slate-700 text-center">
              <FontAwesomeIcon icon={faFilm} className="text-slate-600 text-5xl mb-4" />
              <h3 className="text-xl font-semibold text-slate-100 mb-2">No Profiles</h3>
              <p className="text-slate-400">Create a Topaz Video AI profile to get started</p>
            </div>
          ) : (
            profiles.map((profile) => {
              const isExpanded = expandedProfile === profile.id;
              const isEditing = editingProfile === profile.id;
              const associatedShowIds = profileShowAssociations[profile.id] || [];
              const associatedShows = shows.filter(s => associatedShowIds.includes(s.id));
              
              return (
                <div
                  key={profile.id}
                  className="bg-slate-800 rounded-lg p-5 border border-slate-700"
                >
                  <div className="flex items-start justify-between">
                    <div className="flex-1">
                      {isEditing ? (
                        <div className="space-y-3">
                          <div>
                            <label className="block text-sm font-medium text-slate-300 mb-1">Name *</label>
                            <input
                              type="text"
                              value={editingProfileData.name}
                              onChange={(e) => setEditingProfileData({ ...editingProfileData, name: e.target.value })}
                              className="w-full px-3 py-2 bg-slate-900 border border-slate-700 rounded-lg text-slate-100 focus:outline-none focus:border-cyan-500"
                            />
                          </div>
                          <div>
                            <label className="block text-sm font-medium text-slate-300 mb-1">Description</label>
                            <textarea
                              value={editingProfileData.description}
                              onChange={(e) => setEditingProfileData({ ...editingProfileData, description: e.target.value })}
                              className="w-full px-3 py-2 bg-slate-900 border border-slate-700 rounded-lg text-slate-100 focus:outline-none focus:border-cyan-500"
                              rows="2"
                            />
                          </div>
                          <div className="flex gap-2">
                            <button
                              onClick={() => handleSaveProfile(profile.id)}
                              className="px-3 py-1.5 bg-cyan-500 hover:bg-cyan-600 text-white rounded-lg transition-colors text-sm"
                            >
                              <FontAwesomeIcon icon={faSave} className="mr-1" />
                              Save
                            </button>
                            <button
                              onClick={() => {
                                setEditingProfile(null);
                                setEditingProfileData({ name: '', description: '', settings_json: {} });
                              }}
                              className="px-3 py-1.5 bg-slate-700 hover:bg-slate-600 text-white rounded-lg transition-colors text-sm"
                            >
                              <FontAwesomeIcon icon={faTimes} className="mr-1" />
                              Cancel
                            </button>
                          </div>
                        </div>
                      ) : (
                        <>
                          <h3 className="font-semibold text-lg text-slate-100 mb-1">{profile.name}</h3>
                          {profile.description && (
                            <p className="text-slate-400 text-sm mb-2">{profile.description}</p>
                          )}
                          <p className="text-slate-500 text-xs">
                            Created: {new Date(profile.created_at).toLocaleDateString()}
                          </p>
                        </>
                      )}
                    </div>
                    {!isEditing && (
                      <div className="flex gap-2">
                        <button
                          onClick={() => handleEditProfile(profile)}
                          className="text-cyan-400 hover:text-cyan-300 transition-colors"
                          title="Edit profile"
                        >
                          <FontAwesomeIcon icon={faEdit} />
                        </button>
                        <button
                          onClick={() => setExpandedProfile(isExpanded ? null : profile.id)}
                          className="text-slate-400 hover:text-slate-300 transition-colors"
                        >
                          <FontAwesomeIcon icon={isExpanded ? faChevronUp : faChevronDown} />
                        </button>
                        <button
                          onClick={() => handleDeleteProfile(profile.id)}
                          className="text-red-400 hover:text-red-300 transition-colors"
                          title="Delete profile"
                        >
                          <FontAwesomeIcon icon={faTrash} />
                        </button>
                      </div>
                    )}
                  </div>

                  {isExpanded && !isEditing && (
                    <div className="mt-4 pt-4 border-t border-slate-700">
                      <div className="flex items-center justify-between mb-3">
                        <h4 className="text-sm font-semibold text-slate-300">Associated Shows</h4>
                        <button
                          onClick={() => setAddingShowToProfile(addingShowToProfile === profile.id ? null : profile.id)}
                          className="text-xs text-cyan-400 hover:text-cyan-300 transition-colors"
                        >
                          <FontAwesomeIcon icon={faPlus} className="mr-1" />
                          Add Show
                        </button>
                      </div>
                      
                      {addingShowToProfile === profile.id && (
                        <div className="mb-3 p-3 bg-slate-900/50 rounded border border-slate-700">
                          <select
                            onChange={(e) => {
                              if (e.target.value) {
                                handleAssociateShow(profile.id, parseInt(e.target.value));
                                setAddingShowToProfile(null);
                              }
                            }}
                            className="w-full px-3 py-2 bg-slate-900 border border-slate-700 rounded-lg text-slate-100 focus:outline-none focus:border-cyan-500"
                            defaultValue=""
                          >
                            <option value="">Select a show...</option>
                            {shows
                              .filter(s => !associatedShowIds.includes(s.id))
                              .map(show => (
                                <option key={show.id} value={show.id}>{show.name}</option>
                              ))}
                          </select>
                        </div>
                      )}

                      {associatedShows.length === 0 ? (
                        <p className="text-slate-500 text-sm">No shows associated</p>
                      ) : (
                        <div className="space-y-2">
                          {associatedShows.map(show => (
                            <div key={show.id} className="flex items-center justify-between bg-slate-900/50 rounded p-2">
                              <span className="text-slate-300 text-sm">{show.name}</span>
                              <button
                                onClick={() => handleRemoveShowAssociation(profile.id, show.id)}
                                className="text-red-400 hover:text-red-300 transition-colors text-xs"
                              >
                                <FontAwesomeIcon icon={faTimes} />
                              </button>
                            </div>
                          ))}
                        </div>
                      )}
                    </div>
                  )}
                </div>
              );
            })
          )}
        </div>
      )}

      {/* Jobs Tab */}
      {activeTab === 'jobs' && (
        <div className="space-y-4">
          <h2 className="text-xl font-semibold text-slate-100">Upscaling Jobs</h2>
          {jobs.length === 0 ? (
            <div className="bg-slate-800 rounded-lg p-12 border border-slate-700 text-center">
              <FontAwesomeIcon icon={faList} className="text-slate-600 text-5xl mb-4" />
              <h3 className="text-xl font-semibold text-slate-100 mb-2">No Jobs</h3>
              <p className="text-slate-400">Upscaling jobs will appear here when created</p>
            </div>
          ) : (
            jobs.map((job) => (
              <div
                key={job.job_id}
                className="bg-slate-800 rounded-lg p-5 border border-slate-700"
              >
                <div className="flex items-start justify-between mb-3">
                  <div className="flex-1">
                    <h3 className="font-semibold text-slate-100 mb-1">{job.job_id}</h3>
                    <p className="text-slate-400 text-sm">{job.input_file_path}</p>
                    {job.agent_id && (
                      <p className="text-slate-500 text-xs mt-1">Agent: {job.agent_id}</p>
                    )}
                  </div>
                  <span className={`px-2 py-1 text-xs rounded ${
                    job.status === 'completed' ? 'bg-green-500/20 text-green-400' :
                    job.status === 'failed' ? 'bg-red-500/20 text-red-400' :
                    job.status === 'processing' ? 'bg-cyan-500/20 text-cyan-400' :
                    'bg-slate-700/50 text-slate-400'
                  }`}>
                    {job.status}
                  </span>
                </div>
                {job.status === 'processing' || job.status === 'assigned' ? (
                  <div>
                    <div className="flex justify-between text-xs text-slate-400 mb-1">
                      <span>{Math.round(job.progress)}%</span>
                    </div>
                    <div className="w-full bg-slate-900/50 rounded-full h-2">
                      <div
                        className="bg-cyan-500 h-2 rounded-full transition-all"
                        style={{ width: `${Math.min(job.progress, 100)}%` }}
                      />
                    </div>
                  </div>
                ) : null}
              </div>
            ))
          )}
        </div>
      )}
    </div>
  );
}

