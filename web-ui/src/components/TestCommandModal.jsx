import { useState, useRef, useEffect } from 'react';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { faTimes, faPlay, faTrash } from '@fortawesome/free-solid-svg-icons';
import { api } from '../api';
import toast from 'react-hot-toast';

export default function TestCommandModal({ agentId, agentName, isOpen, onClose }) {
  const [command, setCommand] = useState('');
  const [logs, setLogs] = useState([]);
  const [isRunning, setIsRunning] = useState(false);
  const [debugInfo, setDebugInfo] = useState(null);
  const [pollCount, setPollCount] = useState(0);
  const logsEndRef = useRef(null);
  const pollIntervalRef = useRef(null);
  const pollTimeoutRef = useRef(null);

  useEffect(() => {
    if (isOpen) {
      setCommand('');
      setLogs([]);
      setIsRunning(false);
      setDebugInfo(null);
      setPollCount(0);
    } else {
      // Clean up polling when modal closes
      if (pollIntervalRef.current) {
        clearInterval(pollIntervalRef.current);
        pollIntervalRef.current = null;
      }
      if (pollTimeoutRef.current) {
        clearTimeout(pollTimeoutRef.current);
        pollTimeoutRef.current = null;
      }
    }
    
    return () => {
      // Cleanup on unmount
      if (pollIntervalRef.current) {
        clearInterval(pollIntervalRef.current);
      }
      if (pollTimeoutRef.current) {
        clearTimeout(pollTimeoutRef.current);
      }
    };
  }, [isOpen]);

  useEffect(() => {
    // Auto-scroll to bottom when logs update
    logsEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [logs]);

  const handleRunCommand = async () => {
    if (!command.trim()) {
      toast.error('Please enter a command');
      return;
    }

    const commandToRun = command.trim();
    const timestamp = new Date().toLocaleTimeString();
    
    // Add command to logs
    setLogs(prev => [...prev, {
      type: 'command',
      timestamp,
      content: commandToRun,
    }]);

    setIsRunning(true);
    setCommand(''); // Clear input for next command

    try {
      setDebugInfo({ step: 'Creating instruction...', instructionId: null, status: null });
      const result = await api.testAgentCommand(agentId, commandToRun);
      const instructionId = result.instruction_id;
      
      setDebugInfo({ 
        step: 'Instruction created', 
        instructionId, 
        status: 'pending',
        message: 'Waiting for agent to pick up instruction...'
      });
      
      // Poll for instruction result
      let pollAttempts = 0;
      pollIntervalRef.current = setInterval(async () => {
        pollAttempts++;
        setPollCount(pollAttempts);
        
        try {
          const instruction = await api.getInstruction(instructionId);
          
          setDebugInfo({
            step: 'Polling instruction',
            instructionId,
            status: instruction.status,
            pollAttempts,
            assignedTo: instruction.assigned_to_agent_id,
            hasOutput: !!instruction.output,
            errorMessage: instruction.error_message,
            message: `Status: ${instruction.status}, Poll #${pollAttempts}`
          });
          
          if (instruction.status === 'completed' || instruction.status === 'failed') {
            if (pollIntervalRef.current) {
              clearInterval(pollIntervalRef.current);
              pollIntervalRef.current = null;
            }
            if (pollTimeoutRef.current) {
              clearTimeout(pollTimeoutRef.current);
              pollTimeoutRef.current = null;
            }
            setIsRunning(false);
            setDebugInfo(null);
            
            if (instruction.status === 'completed') {
              if (instruction.output) {
                // Add output to logs
                setLogs(prev => [...prev, {
                  type: 'output',
                  timestamp: new Date().toLocaleTimeString(),
                  content: instruction.output,
                }]);
              } else {
                // No output but completed
                setLogs(prev => [...prev, {
                  type: 'output',
                  timestamp: new Date().toLocaleTimeString(),
                  content: '(Command completed with no output)',
                }]);
              }
            } else if (instruction.status === 'failed') {
              // Add error to logs
              setLogs(prev => [...prev, {
                type: 'error',
                timestamp: new Date().toLocaleTimeString(),
                content: instruction.error_message || 'Command failed',
              }]);
            }
          } else if (instruction.status === 'processing' || instruction.status === 'assigned') {
            setDebugInfo({
              step: 'Instruction in progress',
              instructionId,
              status: instruction.status,
              pollAttempts,
              message: `Agent is ${instruction.status === 'processing' ? 'executing' : 'assigned'} command...`
            });
          }
        } catch (err) {
          setDebugInfo({
            step: 'Error polling',
            instructionId,
            status: 'error',
            pollAttempts,
            error: err.message,
            message: `Failed to get instruction: ${err.message}`
          });
          // Continue polling on error, but log it
          console.error('Error polling for instruction:', err);
        }
      }, 500); // Poll every 500ms
      
      // Stop polling after 30 seconds
      pollTimeoutRef.current = setTimeout(() => {
        if (pollIntervalRef.current) {
          clearInterval(pollIntervalRef.current);
          pollIntervalRef.current = null;
        }
        setIsRunning(false);
        setLogs(prev => [...prev, {
          type: 'error',
          timestamp: new Date().toLocaleTimeString(),
          content: 'Timeout: Command did not complete within 30 seconds. The agent may not be processing instructions.',
        }]);
        setDebugInfo(prev => prev ? {
          ...prev,
          step: 'Timeout',
          message: 'Command timed out after 30 seconds. Check if agent is connected and processing instructions.'
        } : null);
      }, 30000);
      
    } catch (err) {
      setIsRunning(false);
      setDebugInfo({
        step: 'Failed to create instruction',
        error: err.message,
        message: `Error: ${err.message}`
      });
      // Add error message
      setLogs(prev => [...prev, {
        type: 'error',
        timestamp: new Date().toLocaleTimeString(),
        content: `Error creating instruction: ${err.message}`,
      }]);
      toast.error(`Failed to send command: ${err.message}`);
    }
  };

  const handleClearLogs = () => {
    setLogs([]);
  };

  const handleKeyPress = (e) => {
    if (e.key === 'Enter' && !e.shiftKey && !isRunning) {
      e.preventDefault();
      handleRunCommand();
    }
  };

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm">
      <div className="bg-slate-800 rounded-lg border border-slate-700 shadow-2xl w-full max-w-4xl max-h-[90vh] flex flex-col">
        {/* Header */}
        <div className="flex items-center justify-between p-6 border-b border-slate-700">
          <div>
            <h2 className="text-2xl font-bold text-slate-100">Test Agent Command</h2>
            <p className="text-sm text-slate-400 mt-1">Agent: {agentName}</p>
          </div>
          <button
            onClick={onClose}
            className="text-slate-400 hover:text-slate-200 transition-colors p-2 hover:bg-slate-700 rounded"
            title="Close"
          >
            <FontAwesomeIcon icon={faTimes} className="text-xl" />
          </button>
        </div>

        {/* Command Input */}
        <div className="p-6 border-b border-slate-700">
          <label className="block text-sm font-medium text-slate-300 mb-2">
            Command to Run
          </label>
          <div className="flex gap-2">
            <textarea
              value={command}
              onChange={(e) => setCommand(e.target.value)}
              onKeyPress={handleKeyPress}
              placeholder="Enter command to test (e.g., echo 'Hello World', dir, ls, etc.)"
              className="flex-1 px-4 py-2 bg-slate-900 border border-slate-600 rounded-lg text-slate-100 placeholder-slate-500 focus:outline-none focus:border-cyan-500 font-mono text-sm resize-none"
              rows="2"
              disabled={isRunning}
            />
            <button
              onClick={handleRunCommand}
              disabled={isRunning || !command.trim()}
              className="px-6 py-2 bg-cyan-500 hover:bg-cyan-600 disabled:bg-slate-700 disabled:text-slate-500 text-white rounded-lg transition-colors flex items-center gap-2 whitespace-nowrap"
            >
              <FontAwesomeIcon icon={faPlay} />
              Run
            </button>
          </div>
          <p className="text-xs text-slate-500 mt-2">
            Press Enter to run, Shift+Enter for new line. Commands are sent to the agent in real-time.
          </p>
        </div>

        {/* Debug Info Alert */}
        {debugInfo && (
          <div className={`mx-6 mb-4 p-4 rounded-lg border ${
            debugInfo.status === 'error' || debugInfo.step === 'Timeout' || debugInfo.step === 'Failed to create instruction'
              ? 'bg-red-500/10 border-red-500/30 text-red-400'
              : debugInfo.status === 'completed'
              ? 'bg-green-500/10 border-green-500/30 text-green-400'
              : debugInfo.status === 'processing' || debugInfo.status === 'assigned'
              ? 'bg-yellow-500/10 border-yellow-500/30 text-yellow-400'
              : 'bg-blue-500/10 border-blue-500/30 text-blue-400'
          }`}>
            <div className="flex items-start justify-between">
              <div className="flex-1">
                <div className="font-semibold mb-1">{debugInfo.step}</div>
                {debugInfo.message && (
                  <div className="text-sm opacity-90">{debugInfo.message}</div>
                )}
                <div className="text-xs mt-2 space-y-1 opacity-75">
                  {debugInfo.instructionId && (
                    <div>Instruction ID: {debugInfo.instructionId}</div>
                  )}
                  {debugInfo.status && (
                    <div>Status: <span className="font-mono">{debugInfo.status}</span></div>
                  )}
                  {debugInfo.pollAttempts && (
                    <div>Poll attempts: {debugInfo.pollAttempts}</div>
                  )}
                  {debugInfo.assignedTo && (
                    <div>Assigned to: {debugInfo.assignedTo}</div>
                  )}
                  {debugInfo.hasOutput !== undefined && (
                    <div>Has output: {debugInfo.hasOutput ? 'Yes' : 'No'}</div>
                  )}
                  {debugInfo.error && (
                    <div className="text-red-400">Error: {debugInfo.error}</div>
                  )}
                  {debugInfo.errorMessage && (
                    <div className="text-red-400">Error message: {debugInfo.errorMessage}</div>
                  )}
                </div>
              </div>
              <button
                onClick={() => setDebugInfo(null)}
                className="text-slate-400 hover:text-slate-200 ml-4"
              >
                <FontAwesomeIcon icon={faTimes} />
              </button>
            </div>
          </div>
        )}

        {/* Log Output */}
        <div className="flex-1 overflow-hidden flex flex-col p-6">
          <div className="flex items-center justify-between mb-3">
            <label className="block text-sm font-medium text-slate-300">
              Output Log
            </label>
            {logs.length > 0 && (
              <button
                onClick={handleClearLogs}
                className="text-xs text-slate-400 hover:text-slate-200 transition-colors flex items-center gap-1"
              >
                <FontAwesomeIcon icon={faTrash} />
                Clear
              </button>
            )}
          </div>
          <div className="flex-1 bg-slate-900 rounded-lg border border-slate-700 p-4 overflow-y-auto font-mono text-sm">
            {logs.length === 0 ? (
              <div className="text-slate-500 text-center py-8">
                No commands run yet. Enter a command above and click Run.
              </div>
            ) : (
              <div className="space-y-2">
                {logs.map((log, index) => (
                  <div
                    key={index}
                    className={`${
                      log.type === 'command'
                        ? 'text-cyan-400'
                        : log.type === 'output'
                        ? 'text-green-400'
                        : log.type === 'error'
                        ? 'text-red-400'
                        : 'text-slate-300'
                    } whitespace-pre-wrap break-words`}
                  >
                    <span className="text-slate-500">[{log.timestamp}] </span>
                    {log.type === 'command' && <span className="text-slate-400">$ </span>}
                    {log.content}
                  </div>
                ))}
                {isRunning && (
                  <div className="text-slate-500 text-sm italic">
                    Waiting for command output...
                  </div>
                )}
                <div ref={logsEndRef} />
              </div>
            )}
          </div>
        </div>

        {/* Footer */}
        <div className="p-6 border-t border-slate-700 flex justify-end">
          <button
            onClick={onClose}
            className="px-6 py-2 bg-slate-700 hover:bg-slate-600 text-white rounded-lg transition-colors"
          >
            Close
          </button>
        </div>
      </div>
    </div>
  );
}
