import { useState, useEffect, useCallback, useMemo } from 'react';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import {
  faExclamationTriangle,
  faCircleCheck,
  faSpinner,
  faFilter,
  faTriangleExclamation,
  faCircleXmark,
  faBug,
  faCompactDisc,
  faNetworkWired,
  faFileExport,
} from '@fortawesome/free-solid-svg-icons';
import toast from 'react-hot-toast';
import { api } from '../api';

export default function Issues() {
  const [issues, setIssues] = useState([]);
  const [loading, setLoading] = useState(true);
  const [filter, setFilter] = useState('all'); // all, active, resolved
  const [typeFilter, setTypeFilter] = useState('all'); // all, drive_error, network_error, rip_error
  const [expandedIssue, setExpandedIssue] = useState(null);
  const [issueLogs, setIssueLogs] = useState({});
  const [issueNotes, setIssueNotes] = useState({});
  const [newNote, setNewNote] = useState('');
  const [addingNote, setAddingNote] = useState(false);

  useEffect(() => {
    fetchIssues();
  }, []);

  const fetchIssues = useCallback(async () => {
    try {
      setLoading(true);
      const data = await api.getIssues();
      setIssues(data);
    } catch (err) {
      console.error('Failed to fetch issues:', err);
      toast.error('Failed to load issues');
    } finally {
      setLoading(false);
    }
  }, []);

  const handleResolveIssue = useCallback(async (issueId) => {
    try {
      await api.resolveIssue(issueId);
      toast.success('Issue resolved');
      fetchIssues();
    } catch (err) {
      toast.error('Failed to resolve issue: ' + err.message);
    }
  }, [fetchIssues]);

  const handleExportIssues = useCallback(() => {
    const exportData = filteredIssues.map(issue => ({
      id: issue.id,
      timestamp: issue.timestamp,
      category: issue.category,
      message: issue.message,
      drive: issue.drive,
      disc: issue.disc,
      resolved: issue.resolved,
      resolved_at: issue.resolved_at,
    }));

    const blob = new Blob([JSON.stringify(exportData, null, 2)], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `ripley-issues-${new Date().toISOString().split('T')[0]}.json`;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);
    toast.success('Issues exported');
  }, [filteredIssues]);

  const fetchIssueNotes = useCallback(async (issueId) => {
    try {
      const notes = await api.getIssueNotes(issueId);
      setIssueNotes(prev => ({ ...prev, [issueId]: notes }));
    } catch (err) {
      console.error('Failed to fetch notes:', err);
    }
  }, []);

  const handleAddNote = useCallback(async (issueId) => {
    if (!newNote.trim()) return;
    
    try {
      setAddingNote(true);
      await api.addIssueNote(issueId, newNote.trim());
      setNewNote('');
      toast.success('Note added');
      fetchIssueNotes(issueId);
    } catch (err) {
      toast.error('Failed to add note: ' + err.message);
    } finally {
      setAddingNote(false);
    }
  }, [newNote, fetchIssueNotes]);

  const handleDeleteNote = useCallback(async (issueId, noteId) => {
    if (!confirm('Delete this note?')) return;
    
    try {
      await api.deleteIssueNote(issueId, noteId);
      toast.success('Note deleted');
      fetchIssueNotes(issueId);
    } catch (err) {
      toast.error('Failed to delete note: ' + err.message);
    }
  }, [fetchIssueNotes]);

  const toggleIssueLogs = useCallback(async (issue) => {
    if (expandedIssue === issue.id) {
      setExpandedIssue(null);
      return;
    }
    
    setExpandedIssue(issue.id);
    
    // Fetch notes if not already cached
    if (!issueNotes[issue.id]) {
      fetchIssueNotes(issue.id);
    }
    
    // Fetch logs if not already cached
    if (!issueLogs[issue.id]) {
      try {
        // Search for logs around the issue timestamp
        const params = {};
        if (issue.drive) params.drive = issue.drive;
        
        const logs = await api.searchLogs(params);
        
        // Filter logs near the issue timestamp (within 5 minutes)
        const issueTime = new Date(issue.timestamp).getTime();
        const relatedLogs = logs.filter(log => {
          const logTime = new Date(log.timestamp).getTime();
          const diff = Math.abs(logTime - issueTime);
          return diff < 5 * 60 * 1000; // 5 minutes
        }).slice(0, 10); // Limit to 10 logs
        
        setIssueLogs(prev => ({ ...prev, [issue.id]: relatedLogs }));
      } catch (err) {
        console.error('Failed to fetch issue logs:', err);
      }
    }
  }, [expandedIssue, issueLogs, issueNotes, fetchIssueNotes]);

  const getIssueIcon = useCallback((issueType) => {
    switch (issueType?.toLowerCase()) {
      case 'drive_error':
        return faCompactDisc;
      case 'network_error':
        return faNetworkWired;
      case 'rip_error':
        return faCircleXmark;
      default:
        return faBug;
    }
  }, []);

  const getIssueColor = useCallback((issueType) => {
    switch (issueType?.toLowerCase()) {
      case 'drive_error':
        return 'text-yellow-400 bg-yellow-500/10 border-yellow-500/30';
      case 'network_error':
        return 'text-blue-400 bg-blue-500/10 border-blue-500/30';
      case 'rip_error':
        return 'text-red-400 bg-red-500/10 border-red-500/30';
      default:
        return 'text-orange-400 bg-orange-500/10 border-orange-500/30';
    }
  }, []);

  const filteredIssues = useMemo(() => {
    let filtered = issues;
    
    // Filter by status
    switch (filter) {
      case 'active':
        filtered = filtered.filter(i => !i.resolved);
        break;
      case 'resolved':
        filtered = filtered.filter(i => i.resolved);
        break;
    }
    
    // Filter by type
    if (typeFilter !== 'all') {
      filtered = filtered.filter(i => i.issue_type?.toLowerCase() === typeFilter);
    }
    
    return filtered;
  }, [issues, filter, typeFilter]);
  
  // Get unique issue types for filter badges
  const issueTypes = useMemo(() => {
    const types = new Set(issues.map(i => i.issue_type?.toLowerCase()).filter(Boolean));
    return Array.from(types);
  }, [issues]);

  if (loading) {
    return (
      <div className="flex items-center justify-center h-64">
        <FontAwesomeIcon icon={faSpinner} className="text-cyan-400 text-4xl animate-spin" />
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
        <div>
          <h1 className="text-2xl md:text-3xl font-bold text-slate-100">Issues</h1>
          <p className="text-slate-400 mt-2 text-sm sm:text-base">
            View and manage system issues and errors
          </p>
        </div>
        <div className="flex gap-2">
          <button
            onClick={handleExportIssues}
            disabled={filteredIssues.length === 0}
            className="px-4 py-2 bg-slate-700 hover:bg-slate-600 disabled:bg-slate-800 disabled:text-slate-600 text-white rounded-lg transition-colors flex items-center"
            title="Export issues to JSON"
          >
            <FontAwesomeIcon icon={faFileExport} className="mr-2" />
            Export
          </button>
          <button
            onClick={fetchIssues}
            className="px-4 py-2 bg-cyan-500 hover:bg-cyan-600 text-white rounded-lg transition-colors whitespace-nowrap"
          >
            Refresh
          </button>
        </div>
      </div>

      {/* Filter Tabs and Type Badges */}
      <div className="space-y-3">
        <div className="flex gap-2 border-b border-slate-700 overflow-x-auto">
          <button
            onClick={() => setFilter('all')}
            className={`px-4 py-2 font-medium transition-colors whitespace-nowrap text-sm sm:text-base ${
              filter === 'all'
                ? 'text-cyan-400 border-b-2 border-cyan-400'
                : 'text-slate-400 hover:text-slate-300'
            }`}
          >
            All Issues ({issues.length})
          </button>
        <button
          onClick={() => setFilter('active')}
          className={`px-4 py-2 font-medium transition-colors whitespace-nowrap text-sm sm:text-base ${
            filter === 'active'
              ? 'text-cyan-400 border-b-2 border-cyan-400'
              : 'text-slate-400 hover:text-slate-300'
          }`}
        >
          Active ({issues.filter(i => !i.resolved).length})
        </button>
        <button
          onClick={() => setFilter('resolved')}
          className={`px-4 py-2 font-medium transition-colors whitespace-nowrap text-sm sm:text-base ${
            filter === 'resolved'
              ? 'text-cyan-400 border-b-2 border-cyan-400'
              : 'text-slate-400 hover:text-slate-300'
          }`}
        >
          Resolved ({issues.filter(i => i.resolved).length})
        </button>
        </div>
        
        {/* Type Filter Badges */}
        {issueTypes.length > 0 && (
          <div className="flex flex-wrap gap-2">
            <button
              onClick={() => setTypeFilter('all')}
              className={`px-3 py-1 rounded-full text-sm transition-colors ${
                typeFilter === 'all'
                  ? 'bg-cyan-500 text-white'
                  : 'bg-slate-700 text-slate-300 hover:bg-slate-600'
              }`}
            >
              All Types
            </button>
            {issueTypes.map(type => (
              <button
                key={type}
                onClick={() => setTypeFilter(type)}
                className={`px-3 py-1 rounded-full text-sm transition-colors ${
                  typeFilter === type
                    ? 'bg-cyan-500 text-white'
                    : 'bg-slate-700 text-slate-300 hover:bg-slate-600'
                }`}
              >
                <FontAwesomeIcon icon={getIssueIcon(type)} className="mr-1" />
                {type.replace('_', ' ')}
              </button>
            ))}
          </div>
        )}
      </div>

      {/* Issues List */}
      {filteredIssues.length === 0 ? (
        <div className="bg-slate-800 rounded-lg p-12 border border-slate-700 text-center">
          <FontAwesomeIcon icon={faCircleCheck} className="text-green-400 text-5xl mb-4" />
          <h3 className="text-xl font-semibold text-slate-100 mb-2">
            {filter === 'active' ? 'No Active Issues' : filter === 'resolved' ? 'No Resolved Issues' : 'No Issues'}
          </h3>
          <p className="text-slate-400">
            {filter === 'active' 
              ? 'All systems running smoothly!' 
              : filter === 'resolved'
              ? 'No resolved issues to show'
              : 'No issues have been recorded'}
          </p>
        </div>
      ) : (
        <div className="space-y-3">
          {filteredIssues.map((issue) => (
            <div
              key={issue.id}
              className={`rounded-lg p-5 border ${getIssueColor(issue.issue_type)}`}
            >
              <div className="flex items-start justify-between">
                <div className="flex-1">
                  <div className="flex flex-wrap items-center gap-2 mb-2">
                    <FontAwesomeIcon
                      icon={getIssueIcon(issue.issue_type)}
                      className="mr-1"
                    />
                    <h3 className="font-semibold text-base sm:text-lg">{issue.title}</h3>
                    <span className="px-2 py-1 text-xs rounded bg-slate-900/50">
                      {issue.issue_type}
                    </span>
                    {issue.resolved && (
                      <span className="px-2 py-1 text-xs rounded bg-green-500/20 text-green-400">
                        Resolved
                      </span>
                    )}
                  </div>
                  
                  <p className="text-slate-300 mb-3">{issue.description}</p>
                  
                  <div className="flex flex-wrap gap-4 text-sm text-slate-400">
                    <div>
                      <span className="font-medium">Time:</span>{' '}
                      {new Date(issue.timestamp).toLocaleString()}
                    </div>
                    {issue.drive && (
                      <div>
                        <span className="font-medium">Drive:</span> {issue.drive}
                      </div>
                    )}
                    {issue.disc && (
                      <div>
                        <span className="font-medium">Disc:</span> {issue.disc}
                      </div>
                    )}
                    {issue.resolved_at && (
                      <div>
                        <span className="font-medium">Resolved:</span>{' '}
                        {new Date(issue.resolved_at).toLocaleString()}
                      </div>
                    )}
                  </div>
                  
                  {/* Show Related Logs Button */}
                  <button
                    onClick={() => toggleIssueLogs(issue)}
                    className="mt-3 text-sm text-cyan-400 hover:text-cyan-300 transition-colors"
                  >
                    {expandedIssue === issue.id ? '▼' : '▶'} Show Related Logs
                  </button>
                  
                  {/* Notes Section */}
                  {expandedIssue === issue.id && (
                    <div className="mt-3 bg-slate-900/50 rounded p-3">
                      <h4 className="text-sm font-semibold text-slate-300 mb-2">Notes & Comments</h4>
                      
                      {/* Existing Notes */}
                      {issueNotes[issue.id] && issueNotes[issue.id].length > 0 && (
                        <div className="space-y-2 mb-3">
                          {issueNotes[issue.id].map((note) => (
                            <div key={note.id} className="bg-slate-800 rounded p-2 text-sm">
                              <div className="flex justify-between items-start">
                                <p className="text-slate-300 flex-1">{note.note}</p>
                                <button
                                  onClick={() => handleDeleteNote(issue.id, note.id)}
                                  className="text-red-400 hover:text-red-300 ml-2"
                                  title="Delete note"
                                >
                                  ×
                                </button>
                              </div>
                              <p className="text-xs text-slate-500 mt-1">
                                {new Date(note.timestamp).toLocaleString()}
                              </p>
                            </div>
                          ))}
                        </div>
                      )}
                      
                      {/* Add Note Form */}
                      <div className="flex gap-2">
                        <input
                          type="text"
                          value={newNote}
                          onChange={(e) => setNewNote(e.target.value)}
                          onKeyPress={(e) => e.key === 'Enter' && handleAddNote(issue.id)}
                          placeholder="Add a note..."
                          className="flex-1 px-3 py-1.5 bg-slate-800 border border-slate-700 rounded text-slate-100 text-sm placeholder-slate-500 focus:outline-none focus:border-cyan-500"
                        />
                        <button
                          onClick={() => handleAddNote(issue.id)}
                          disabled={!newNote.trim() || addingNote}
                          className="px-3 py-1.5 bg-cyan-600 hover:bg-cyan-500 disabled:bg-slate-700 disabled:text-slate-500 text-white rounded text-sm transition-colors"
                        >
                          {addingNote ? 'Adding...' : 'Add'}
                        </button>
                      </div>
                    </div>
                  )}
                  
                  {/* Related Logs */}
                  {expandedIssue === issue.id && (
                    <div className="mt-3 bg-slate-900/50 rounded p-3 max-h-64 overflow-y-auto">
                      <h4 className="text-sm font-semibold text-slate-300 mb-2">Related Logs (within 5 min)</h4>
                      {issueLogs[issue.id] ? (
                        issueLogs[issue.id].length > 0 ? (
                          <div className="space-y-1 font-mono text-xs">
                            {issueLogs[issue.id].map((log, idx) => (
                              <div key={idx} className="text-slate-400">
                                <span className="text-slate-600">[{new Date(log.timestamp).toLocaleTimeString()}]</span>
                                {log.drive && <span className="text-slate-600 ml-1">[{log.drive}]</span>}
                                <span className={`ml-1 ${
                                  log.level === 'error' ? 'text-red-400' :
                                  log.level === 'warning' ? 'text-yellow-400' :
                                  log.level === 'success' ? 'text-green-400' :
                                  'text-cyan-400'
                                }`}>{log.message}</span>
                              </div>
                            ))}
                          </div>
                        ) : (
                          <p className="text-slate-500 text-sm">No related logs found</p>
                        )
                      ) : (
                        <div className="flex items-center text-slate-500">
                          <FontAwesomeIcon icon={faSpinner} className="animate-spin mr-2" />
                          Loading logs...
                        </div>
                      )}
                    </div>
                  )}
                </div>
                
                {!issue.resolved && (
                  <button
                    onClick={() => handleResolveIssue(issue.id)}
                    className="ml-4 px-4 py-2 bg-green-600 hover:bg-green-700 text-white text-sm rounded transition-colors flex items-center"
                  >
                    <FontAwesomeIcon icon={faCircleCheck} className="mr-2" />
                    Resolve
                  </button>
                )}
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
