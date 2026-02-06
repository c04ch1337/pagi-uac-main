import React, { useState, useMemo, useEffect } from 'react';
import { X, Server, Zap, Eye, Upload, Trash2, Image as ImageIcon, Palette, UserCircle, Sun, Moon, Brain, Bot, Sliders, MessageSquare, Terminal, Search, Filter, Calendar, History, Settings2, AlertCircle, Database, CheckCircle2, XCircle, RefreshCw } from 'lucide-react';
import { AppSettings, Message } from '../types';
import LogTerminal from './LogTerminal';

interface KbStatusItem {
  slot_id: number;
  name: string;
  tree_name: string;
  connected: boolean;
  entry_count: number;
  error: string | null;
}

interface KbStatusResponse {
  status: string;
  all_connected: boolean;
  total_entries: number;
  knowledge_bases: KbStatusItem[];
}

interface SettingsSidebarProps {
  isOpen: boolean;
  onClose: () => void;
  settings: AppSettings;
  setSettings: React.Dispatch<React.SetStateAction<AppSettings>>;
  messages: Message[];
  onClearChat: () => void;
}

const SettingsSidebar: React.FC<SettingsSidebarProps> = ({ isOpen, onClose, settings, setSettings, messages, onClearChat }) => {
  const [activeTab, setActiveTab] = useState<'settings' | 'history'>('settings');
  const [searchQuery, setSearchQuery] = useState('');
  const [roleFilter, setRoleFilter] = useState<'all' | 'user' | 'agi'>('all');
  const [timeFilter, setTimeFilter] = useState<'all' | '24h' | '7d'>('all');
  const [uploadErrors, setUploadErrors] = useState<Record<string, string>>({});
  const [kbStatus, setKbStatus] = useState<KbStatusResponse | null>(null);
  const [kbLoading, setKbLoading] = useState(false);
  const [kbError, setKbError] = useState<string | null>(null);

  const logsUrl = useMemo(() => {
    // Preferred: just swap the chat endpoint for the logs SSE endpoint
    if (settings.apiUrl.includes('/api/v1/chat')) {
      return settings.apiUrl.replace(/\/api\/v1\/chat\/?$/, '/api/v1/logs');
    }

    // Fallback: if the URL is parseable, use its origin.
    try {
      const u = new URL(settings.apiUrl);
      return `${u.origin}/api/v1/logs`;
    } catch {
      // Last resort: keep LogTerminal default
      return undefined;
    }
  }, [settings.apiUrl]);

  // Fetch KB status when sidebar opens
  const fetchKbStatus = async () => {
    setKbLoading(true);
    setKbError(null);
    try {
      const baseUrl = settings.apiUrl.replace('/api/v1/chat', '');
      const response = await fetch(`${baseUrl}/api/v1/kb-status`);
      if (!response.ok) throw new Error(`HTTP ${response.status}`);
      const data: KbStatusResponse = await response.json();
      setKbStatus(data);
    } catch (err) {
      setKbError(err instanceof Error ? err.message : 'Failed to fetch KB status');
    } finally {
      setKbLoading(false);
    }
  };

  useEffect(() => {
    if (isOpen && activeTab === 'settings') {
      fetchKbStatus();
    }
  }, [isOpen, activeTab, settings.apiUrl]);

  // Filter messages logic
  const filteredMessages = useMemo(() => {
    if (activeTab !== 'history') return [];
    
    const now = Date.now();
    const oneDay = 24 * 60 * 60 * 1000;
    const sevenDays = 7 * oneDay;

    return messages.filter(msg => {
      // Role Filter
      if (roleFilter !== 'all' && msg.role !== roleFilter) return false;
      
      // Time Filter
      if (timeFilter === '24h' && (now - msg.timestamp) > oneDay) return false;
      if (timeFilter === '7d' && (now - msg.timestamp) > sevenDays) return false;

      // Text Search
      if (searchQuery.trim()) {
        const query = searchQuery.toLowerCase();
        const contentMatch = msg.content.toLowerCase().includes(query);
        const thoughtMatch = msg.thoughts?.some(t => 
            t.title.toLowerCase().includes(query) || t.content.toLowerCase().includes(query)
        );
        return contentMatch || thoughtMatch;
      }

      return true;
    }).reverse(); // Show newest first
  }, [messages, activeTab, searchQuery, roleFilter, timeFilter]);

  if (!isOpen) return null;

  const handleFileUpload = (e: React.ChangeEvent<HTMLInputElement>, key: 'customLogo' | 'customFavicon' | 'userAvatar' | 'agiAvatar') => {
    const file = e.target.files?.[0];
    
    // Reset error for this key
    setUploadErrors(prev => {
        const next = { ...prev };
        delete next[key];
        return next;
    });

    if (file) {
      // Validation: File Type
      if (!file.type.startsWith('image/')) {
        setUploadErrors(prev => ({ ...prev, [key]: 'Invalid file type. Please select an image (PNG, JPG, etc.).' }));
        return;
      }

      // Validation: File Size (1MB)
      if (file.size > 1024 * 1024) {
        setUploadErrors(prev => ({ ...prev, [key]: 'File is too large. Maximum size is 1MB.' }));
        return;
      }

      const reader = new FileReader();
      reader.onloadend = () => {
        setSettings(prev => ({ ...prev, [key]: reader.result as string }));
      };
      reader.onerror = () => {
        setUploadErrors(prev => ({ ...prev, [key]: 'Failed to read file.' }));
      };
      reader.readAsDataURL(file);
    }
  };

  const clearImage = (key: 'customLogo' | 'customFavicon' | 'userAvatar' | 'agiAvatar') => {
    setSettings(prev => ({ ...prev, [key]: undefined }));
    setUploadErrors(prev => {
        const next = { ...prev };
        delete next[key];
        return next;
    });
  };

  return (
    <div className="fixed inset-y-0 right-0 w-80 bg-white dark:bg-zinc-900 border-l border-zinc-200 dark:border-zinc-800 shadow-2xl transform transition-transform duration-300 z-50 overflow-hidden flex flex-col">
      {/* Sidebar Header */}
      <div className="flex items-center justify-between p-4 border-b border-zinc-200 dark:border-zinc-800 bg-white dark:bg-zinc-900 z-10 shrink-0">
        <h2 className="text-zinc-900 dark:text-zinc-100 font-medium flex items-center gap-2">
          {activeTab === 'settings' ? <Server size={18} /> : <History size={18} />}
          {activeTab === 'settings' ? 'Configuration' : 'Chat History'}
        </h2>
        <button 
          onClick={onClose}
          className="text-zinc-400 hover:text-zinc-900 dark:hover:text-white transition-colors"
        >
          <X size={20} />
        </button>
      </div>

      {/* Tab Switcher */}
      <div className="grid grid-cols-2 p-2 gap-2 bg-zinc-50 dark:bg-zinc-950 border-b border-zinc-200 dark:border-zinc-800 shrink-0">
         <button
            onClick={() => setActiveTab('settings')}
            className={`flex items-center justify-center gap-2 py-2 text-xs font-medium rounded transition-all ${
                activeTab === 'settings'
                ? 'bg-white dark:bg-zinc-800 text-zinc-900 dark:text-zinc-100 shadow-sm border border-zinc-200 dark:border-zinc-700'
                : 'text-zinc-500 dark:text-zinc-400 hover:bg-zinc-200/50 dark:hover:bg-zinc-800/50'
            }`}
         >
            <Settings2 size={14} />
            Settings
         </button>
         <button
            onClick={() => setActiveTab('history')}
             className={`flex items-center justify-center gap-2 py-2 text-xs font-medium rounded transition-all ${
                activeTab === 'history'
                ? 'bg-white dark:bg-zinc-800 text-zinc-900 dark:text-zinc-100 shadow-sm border border-zinc-200 dark:border-zinc-700'
                : 'text-zinc-500 dark:text-zinc-400 hover:bg-zinc-200/50 dark:hover:bg-zinc-800/50'
            }`}
         >
            <History size={14} />
            History ({messages.length})
         </button>
      </div>

      {/* Content Area */}
      <div className="flex-1 overflow-y-auto">
        {activeTab === 'settings' ? (
            <div className="p-6 space-y-6">
                
                {/* Theme Selector */}
                <div className="p-1 bg-zinc-100 dark:bg-zinc-950 rounded-lg flex border border-zinc-200 dark:border-zinc-800">
                <button
                    onClick={() => setSettings(prev => ({ ...prev, theme: 'light' }))}
                    className={`flex-1 flex items-center justify-center gap-2 py-1.5 text-xs font-medium rounded-md transition-all ${
                    settings.theme === 'light' 
                        ? 'bg-white shadow text-zinc-900' 
                        : 'text-zinc-500 hover:text-zinc-900 dark:hover:text-zinc-300'
                    }`}
                >
                    <Sun size={14} />
                    Light
                </button>
                <button
                    onClick={() => setSettings(prev => ({ ...prev, theme: 'dark' }))}
                    className={`flex-1 flex items-center justify-center gap-2 py-1.5 text-xs font-medium rounded-md transition-all ${
                    settings.theme === 'dark' 
                        ? 'bg-zinc-800 shadow text-white' 
                        : 'text-zinc-500 hover:text-zinc-900 dark:hover:text-zinc-300'
                    }`}
                >
                    <Moon size={14} />
                    Dark
                </button>
                </div>

                {/* User Profile Section */}
                <div className="space-y-4 pt-4 border-t border-zinc-200 dark:border-zinc-800">
                    <h3 className="text-xs font-semibold text-zinc-500 uppercase tracking-wider flex items-center gap-2">
                        <UserCircle size={14} />
                        User Profile
                    </h3>

                    <div className="flex items-start gap-3">
                        {/* Avatar Preview */}
                        <div className="w-12 h-12 bg-zinc-100 dark:bg-zinc-950 border border-zinc-200 dark:border-zinc-800 rounded-full flex items-center justify-center overflow-hidden shrink-0 relative group shadow-sm">
                            {settings.userAvatar ? (
                            <>
                                <img src={settings.userAvatar} alt="Avatar" className="w-full h-full object-cover" />
                                <button 
                                onClick={() => clearImage('userAvatar')}
                                className="absolute inset-0 bg-black/60 flex items-center justify-center opacity-0 group-hover:opacity-100 transition-opacity text-white"
                                title="Remove avatar"
                                >
                                <Trash2 size={16} />
                                </button>
                            </>
                            ) : (
                            <UserCircle size={24} className="text-zinc-300 dark:text-zinc-700" />
                            )}
                        </div>

                        <div className="flex-1 space-y-2">
                            {/* Avatar Upload Drop Zone */}
                            <div className="relative">
                                <label 
                                    className={`cursor-pointer flex flex-col items-center justify-center gap-1.5 px-3 py-2 bg-zinc-50 dark:bg-zinc-900/50 hover:bg-zinc-100 dark:hover:bg-zinc-800 border-2 border-dashed rounded-lg transition-all duration-200 group/label ${
                                        uploadErrors['userAvatar'] 
                                        ? 'border-red-300 dark:border-red-900/50 bg-red-50/50 dark:bg-red-900/10' 
                                        : 'border-zinc-200 dark:border-zinc-800 hover:border-zinc-300 dark:hover:border-zinc-700'
                                    }`}
                                >
                                    <div className="flex items-center gap-2 text-zinc-500 dark:text-zinc-400 group-hover/label:text-zinc-700 dark:group-hover/label:text-zinc-200 transition-colors">
                                        <Upload size={14} />
                                        <span className="text-xs font-medium">Click to upload image</span>
                                    </div>
                                    <span className="text-[10px] text-zinc-400 dark:text-zinc-600">Max 1MB (PNG, JPG)</span>
                                    <input
                                        type="file"
                                        accept="image/*"
                                        className="hidden"
                                        onChange={(e) => handleFileUpload(e, 'userAvatar')}
                                    />
                                </label>
                                {uploadErrors['userAvatar'] && (
                                    <div className="flex items-center gap-1.5 mt-1.5 text-red-500 dark:text-red-400 text-[10px] animate-in slide-in-from-top-1">
                                        <AlertCircle size={10} />
                                        <span>{uploadErrors['userAvatar']}</span>
                                    </div>
                                )}
                            </div>

                            {/* Name Input */}
                            <div className="space-y-1">
                                <label className="text-[10px] text-zinc-400 dark:text-zinc-500 uppercase tracking-wider font-semibold">Display Name</label>
                                <input
                                type="text"
                                value={settings.userAlias || ''}
                                onChange={(e) => setSettings(prev => ({ ...prev, userAlias: e.target.value }))}
                                className="w-full bg-zinc-50 dark:bg-zinc-950 border border-zinc-300 dark:border-zinc-800 rounded px-2 py-1.5 text-zinc-900 dark:text-zinc-300 text-xs focus:outline-none focus:border-zinc-400 dark:focus:border-zinc-600 focus:ring-1 focus:ring-zinc-400 dark:focus:ring-zinc-600 transition-all"
                                placeholder="e.g. Operator"
                                />
                            </div>
                        </div>
                    </div>
                </div>

                {/* Orchestrator Persona & Agent Settings */}
                <div className="space-y-4 pt-4 border-t border-zinc-200 dark:border-zinc-800">
                    <h3 className="text-xs font-semibold text-zinc-500 uppercase tracking-wider flex items-center gap-2">
                        <Bot size={14} />
                        Agent & Persona
                    </h3>

                    {/* AGI Avatar Upload */}
                    <div className="flex items-start gap-3">
                        <div className="w-10 h-10 bg-zinc-50 dark:bg-zinc-950 border border-zinc-200 dark:border-zinc-800 rounded-lg flex items-center justify-center overflow-hidden shrink-0 relative group shadow-sm mt-1">
                            {settings.agiAvatar ? (
                            <>
                                <img src={settings.agiAvatar} alt="AGI Avatar" className="w-full h-full object-cover" />
                                <button 
                                onClick={() => clearImage('agiAvatar')}
                                className="absolute inset-0 bg-black/60 flex items-center justify-center opacity-0 group-hover:opacity-100 transition-opacity text-white"
                                title="Remove AGI avatar"
                                >
                                <Trash2 size={16} />
                                </button>
                            </>
                            ) : (
                            <Bot size={20} className="text-zinc-400 dark:text-zinc-600" />
                            )}
                        </div>
                        <div className="flex-1">
                            <label className="text-[10px] text-zinc-400 dark:text-zinc-500 uppercase tracking-wider font-semibold block mb-1">Agent Avatar</label>
                            
                             <div className="relative mb-2">
                                <label 
                                    className={`cursor-pointer flex flex-col items-center justify-center gap-1.5 px-3 py-2 bg-zinc-50 dark:bg-zinc-900/50 hover:bg-zinc-100 dark:hover:bg-zinc-800 border-2 border-dashed rounded-lg transition-all duration-200 group/label ${
                                        uploadErrors['agiAvatar'] 
                                        ? 'border-red-300 dark:border-red-900/50 bg-red-50/50 dark:bg-red-900/10' 
                                        : 'border-zinc-200 dark:border-zinc-800 hover:border-zinc-300 dark:hover:border-zinc-700'
                                    }`}
                                >
                                    <div className="flex items-center gap-2 text-zinc-500 dark:text-zinc-400 group-hover/label:text-zinc-700 dark:group-hover/label:text-zinc-200 transition-colors">
                                        <Upload size={14} />
                                        <span className="text-xs font-medium">Upload Icon</span>
                                    </div>
                                    <span className="text-[10px] text-zinc-400 dark:text-zinc-600">Max 1MB (PNG, JPG)</span>
                                    <input
                                        type="file"
                                        accept="image/*"
                                        className="hidden"
                                        onChange={(e) => handleFileUpload(e, 'agiAvatar')}
                                    />
                                </label>
                                {uploadErrors['agiAvatar'] && (
                                    <div className="flex items-center gap-1.5 mt-1.5 text-red-500 dark:text-red-400 text-[10px] animate-in slide-in-from-top-1">
                                        <AlertCircle size={10} />
                                        <span>{uploadErrors['agiAvatar']}</span>
                                    </div>
                                )}
                            </div>

                            {/* URL Input */}
                            <input
                                type="text"
                                value={settings.agiAvatar?.startsWith('data:') ? '' : settings.agiAvatar || ''}
                                onChange={(e) => setSettings(prev => ({ ...prev, agiAvatar: e.target.value }))}
                                className="w-full bg-zinc-50 dark:bg-zinc-950 border border-zinc-300 dark:border-zinc-800 rounded px-2 py-1.5 text-zinc-900 dark:text-zinc-300 text-xs focus:outline-none focus:border-zinc-400 dark:focus:border-zinc-600 focus:ring-1 focus:ring-zinc-400 dark:focus:ring-zinc-600 transition-all"
                                placeholder={settings.agiAvatar?.startsWith('data:') ? "Using uploaded image" : "Or paste image URL..."}
                            />
                        </div>
                    </div>

                    {/* Orchestrator Persona */}
                    <div className="space-y-2">
                        <label className="text-xs text-zinc-400 dark:text-zinc-500">Orchestrator Persona</label>
                        <div className="relative">
                        <select
                            value={settings.orchestratorPersona}
                            onChange={(e) => setSettings(prev => ({ ...prev, orchestratorPersona: e.target.value }))}
                            className="w-full bg-zinc-50 dark:bg-zinc-950 border border-zinc-300 dark:border-zinc-800 rounded px-3 py-2 text-zinc-900 dark:text-zinc-300 text-sm focus:outline-none focus:border-zinc-400 dark:focus:border-zinc-600 focus:ring-1 focus:ring-zinc-400 dark:focus:ring-zinc-600 appearance-none cursor-pointer transition-all"
                        >
                            <option value="general_assistant">General Assistant</option>
                            <option value="researcher">Deep Researcher</option>
                            <option value="coder">Senior Developer</option>
                            <option value="creative">Creative Writer</option>
                            <option value="analyst">Data Analyst</option>
                            <option value="socratic">Socratic Tutor</option>
                        </select>
                        <div className="absolute right-3 top-1/2 -translate-y-1/2 pointer-events-none text-zinc-500">
                            <Brain size={14} />
                        </div>
                        </div>
                    </div>

                    {/* LLM Model Name */}
                    <div className="space-y-2">
                        <label className="text-xs text-zinc-400 dark:text-zinc-500">Model ID</label>
                        <div className="relative">
                        <input
                            type="text"
                            list="model-suggestions"
                            value={settings.llmModel}
                            onChange={(e) => setSettings(prev => ({ ...prev, llmModel: e.target.value }))}
                            className="w-full bg-zinc-50 dark:bg-zinc-950 border border-zinc-300 dark:border-zinc-800 rounded px-3 py-2 text-zinc-900 dark:text-zinc-300 text-sm focus:outline-none focus:border-zinc-400 dark:focus:border-zinc-600 focus:ring-1 focus:ring-zinc-400 dark:focus:ring-zinc-600 transition-all font-mono"
                            placeholder="e.g. gpt-4o, llama3-70b"
                        />
                        <datalist id="model-suggestions">
                            <option value="gpt-4o" />
                            <option value="claude-3-5-sonnet-20240620" />
                            <option value="llama3-70b-8192" />
                            <option value="mixtral-8x7b-32768" />
                            <option value="gemini-1.5-pro" />
                        </datalist>
                        </div>
                    </div>

                    {/* Temperature */}
                    <div className="space-y-2">
                        <div className="flex justify-between items-center">
                        <label className="text-xs text-zinc-400 dark:text-zinc-500 flex items-center gap-1">
                            <Sliders size={12} />
                            Temperature
                        </label>
                        <span className="text-xs font-mono text-zinc-600 dark:text-zinc-400 bg-zinc-100 dark:bg-zinc-800 px-1.5 py-0.5 rounded">
                            {settings.llmTemperature}
                        </span>
                        </div>
                        <input
                        type="range"
                        min="0"
                        max="2"
                        step="0.1"
                        value={settings.llmTemperature}
                        onChange={(e) => setSettings(prev => ({ ...prev, llmTemperature: parseFloat(e.target.value) }))}
                        className="w-full h-1.5 bg-zinc-200 dark:bg-zinc-800 rounded-lg appearance-none cursor-pointer accent-orange-500"
                        />
                        <div className="flex justify-between text-[10px] text-zinc-400">
                        <span>Precise</span>
                        <span>Creative</span>
                        </div>
                    </div>

                    {/* Max Tokens */}
                    <div className="space-y-2">
                        <label className="text-xs text-zinc-400 dark:text-zinc-500 flex items-center gap-1">
                            <MessageSquare size={12} />
                            Max Tokens
                        </label>
                        <input
                        type="number"
                        value={settings.llmMaxTokens}
                        onChange={(e) => setSettings(prev => ({ ...prev, llmMaxTokens: parseInt(e.target.value) || 0 }))}
                        className="w-full bg-zinc-50 dark:bg-zinc-950 border border-zinc-300 dark:border-zinc-800 rounded px-3 py-2 text-zinc-900 dark:text-zinc-300 text-sm focus:outline-none focus:border-zinc-400 dark:focus:border-zinc-600 focus:ring-1 focus:ring-zinc-400 dark:focus:ring-zinc-600 transition-all font-mono"
                        />
                    </div>
                </div>

                {/* API URL Configuration */}
                <div className="space-y-2 pt-4 border-t border-zinc-200 dark:border-zinc-800">
                <label className="text-xs uppercase tracking-wider text-zinc-500 font-semibold flex items-center gap-2">
                    <Server size={14} />
                    Orchestrator Endpoint
                </label>
                <input
                    type="text"
                    value={settings.apiUrl}
                    onChange={(e) => setSettings(prev => ({ ...prev, apiUrl: e.target.value }))}
                    className="w-full bg-zinc-50 dark:bg-zinc-950 border border-zinc-300 dark:border-zinc-800 rounded px-3 py-2 text-zinc-900 dark:text-zinc-300 text-sm focus:outline-none focus:border-zinc-400 dark:focus:border-zinc-600 focus:ring-1 focus:ring-zinc-400 dark:focus:ring-zinc-600 transition-all font-mono"
                    placeholder="http://127.0.0.1:8001/api/v1/chat (Gateway)"
                />
                </div>

                {/* Logs */}
                <div className="space-y-3 pt-4 border-t border-zinc-200 dark:border-zinc-800">
                  <h3 className="text-xs font-semibold text-zinc-500 uppercase tracking-wider flex items-center gap-2">
                    <Terminal size={14} />
                    Logs
                  </h3>
                  <p className="text-[10px] text-zinc-500 dark:text-zinc-600 leading-relaxed">
                    Live log stream from the Gateway (Server-Sent Events).
                  </p>
                  <div className="rounded-lg overflow-hidden border border-zinc-200 dark:border-zinc-800">
                    <LogTerminal logsUrl={logsUrl} />
                  </div>
                </div>

                {/* Feature Toggles */}
                <div className="space-y-4 pt-4 border-t border-zinc-200 dark:border-zinc-800">
                <div 
                    className="flex items-center justify-between cursor-pointer group"
                    onClick={() => setSettings(prev => ({ ...prev, stream: !prev.stream }))}
                >
                    <span className="flex items-center gap-2 text-zinc-600 dark:text-zinc-400 group-hover:text-zinc-900 dark:group-hover:text-zinc-200 transition-colors text-sm">
                    <Zap size={16} />
                    Streaming (Experimental)
                    </span>
                    <div className={`w-10 h-5 rounded-full relative transition-colors ${settings.stream ? 'bg-orange-500 dark:bg-orange-900' : 'bg-zinc-300 dark:bg-zinc-800'}`}>
                    <div className={`absolute top-1 w-3 h-3 rounded-full bg-white transition-all duration-200 ${settings.stream ? 'left-6 bg-orange-100 dark:bg-orange-400' : 'left-1 bg-zinc-500'}`} />
                    </div>
                </div>

                <div 
                    className="flex items-center justify-between cursor-pointer group"
                    onClick={() => setSettings(prev => ({ ...prev, showThoughts: !prev.showThoughts }))}
                >
                    <span className="flex items-center gap-2 text-zinc-600 dark:text-zinc-400 group-hover:text-zinc-900 dark:group-hover:text-zinc-200 transition-colors text-sm">
                    <Eye size={16} />
                    Show Reasoning Layers
                    </span>
                    <div className={`w-10 h-5 rounded-full relative transition-colors ${settings.showThoughts ? 'bg-indigo-600 dark:bg-indigo-900' : 'bg-zinc-300 dark:bg-zinc-800'}`}>
                    <div className={`absolute top-1 w-3 h-3 rounded-full bg-white transition-all duration-200 ${settings.showThoughts ? 'left-6 bg-indigo-100 dark:bg-indigo-400' : 'left-1 bg-zinc-500'}`} />
                    </div>
                </div>
                </div>

                {/* Branding Settings */}
                <div className="space-y-4 pt-4 border-t border-zinc-200 dark:border-zinc-800">
                    <h3 className="text-xs font-semibold text-zinc-500 uppercase tracking-wider">Custom Branding</h3>
                    
                    {/* Logo Upload */}
                    <div className="space-y-2">
                        <label className="text-xs text-zinc-400 dark:text-zinc-500">Custom Logo</label>
                        <div className="flex items-center gap-3">
                        <div className="w-10 h-10 bg-zinc-50 dark:bg-zinc-950 border border-zinc-200 dark:border-zinc-800 rounded flex items-center justify-center overflow-hidden shrink-0 relative group shadow-sm">
                            {settings.customLogo ? (
                            <>
                                <img src={settings.customLogo} alt="Logo" className="w-full h-full object-contain p-1" />
                                <button 
                                onClick={() => clearImage('customLogo')}
                                className="absolute inset-0 bg-black/60 flex items-center justify-center opacity-0 group-hover:opacity-100 transition-opacity text-red-400"
                                >
                                <Trash2 size={14} />
                                </button>
                            </>
                            ) : (
                            <ImageIcon size={16} className="text-zinc-400 dark:text-zinc-600" />
                            )}
                        </div>
                        
                        <div className="flex-1 relative">
                             <label className={`cursor-pointer flex items-center justify-center gap-2 bg-zinc-100 dark:bg-zinc-900 border border-zinc-200 dark:border-zinc-800 hover:border-zinc-300 dark:hover:border-zinc-700 hover:bg-white dark:hover:bg-zinc-800/50 text-zinc-700 dark:text-zinc-300 text-xs py-2 px-3 rounded transition-all ${uploadErrors['customLogo'] ? 'border-red-300 dark:border-red-900/50 bg-red-50/20' : ''}`}>
                                <Upload size={14} />
                                <span>Upload Logo</span>
                                <input
                                type="file"
                                accept="image/*"
                                className="hidden"
                                onChange={(e) => handleFileUpload(e, 'customLogo')}
                                />
                            </label>
                             {uploadErrors['customLogo'] && (
                                <div className="flex items-center gap-1.5 mt-1.5 text-red-500 dark:text-red-400 text-[10px] animate-in slide-in-from-top-1">
                                    <AlertCircle size={10} />
                                    <span>{uploadErrors['customLogo']}</span>
                                </div>
                            )}
                        </div>
                        </div>
                    </div>

                    {/* Favicon Upload */}
                    <div className="space-y-2">
                        <label className="text-xs text-zinc-400 dark:text-zinc-500">Custom Favicon</label>
                        <div className="flex items-center gap-3">
                        <div className="w-10 h-10 bg-zinc-50 dark:bg-zinc-950 border border-zinc-200 dark:border-zinc-800 rounded flex items-center justify-center overflow-hidden shrink-0 relative group shadow-sm">
                            {settings.customFavicon ? (
                            <>
                                <img src={settings.customFavicon} alt="Favicon" className="w-full h-full object-contain p-2" />
                                <button 
                                onClick={() => clearImage('customFavicon')}
                                className="absolute inset-0 bg-black/60 flex items-center justify-center opacity-0 group-hover:opacity-100 transition-opacity text-red-400"
                                >
                                <Trash2 size={14} />
                                </button>
                            </>
                            ) : (
                            <ImageIcon size={16} className="text-zinc-400 dark:text-zinc-600" />
                            )}
                        </div>
                        <div className="flex-1 relative">
                             <label className={`cursor-pointer flex items-center justify-center gap-2 bg-zinc-100 dark:bg-zinc-900 border border-zinc-200 dark:border-zinc-800 hover:border-zinc-300 dark:hover:border-zinc-700 hover:bg-white dark:hover:bg-zinc-800/50 text-zinc-700 dark:text-zinc-300 text-xs py-2 px-3 rounded transition-all ${uploadErrors['customFavicon'] ? 'border-red-300 dark:border-red-900/50 bg-red-50/20' : ''}`}>
                                <Upload size={14} />
                                <span>Upload Favicon</span>
                                <input
                                type="file"
                                accept="image/*"
                                className="hidden"
                                onChange={(e) => handleFileUpload(e, 'customFavicon')}
                                />
                            </label>
                             {uploadErrors['customFavicon'] && (
                                <div className="flex items-center gap-1.5 mt-1.5 text-red-500 dark:text-red-400 text-[10px] animate-in slide-in-from-top-1">
                                    <AlertCircle size={10} />
                                    <span>{uploadErrors['customFavicon']}</span>
                                </div>
                            )}
                        </div>
                        </div>
                    </div>
                </div>

                {/* Custom CSS */}
                <div className="space-y-4 pt-4 border-t border-zinc-200 dark:border-zinc-800">
                <h3 className="text-xs font-semibold text-zinc-500 uppercase tracking-wider flex items-center gap-2">
                    <Palette size={12} />
                    Advanced Styling
                </h3>
                <div className="space-y-2">
                    <label className="text-xs text-zinc-400 dark:text-zinc-500">Custom CSS</label>
                    <textarea
                    value={settings.customCss || ''}
                    onChange={(e) => setSettings(prev => ({ ...prev, customCss: e.target.value }))}
                    className="w-full bg-zinc-50 dark:bg-zinc-950 border border-zinc-300 dark:border-zinc-800 rounded px-3 py-2 text-zinc-900 dark:text-zinc-300 text-xs font-mono focus:outline-none focus:border-zinc-400 dark:focus:border-zinc-600 focus:ring-1 focus:ring-zinc-400 dark:focus:ring-zinc-600 transition-all h-32 resize-y"
                    placeholder=".bg-zinc-950 { background-color: #000; }"
                    spellCheck={false}
                    />
                    <p className="text-[10px] text-zinc-500 dark:text-zinc-600">
                    Override global styles. Changes apply immediately.
                    </p>
                </div>
                </div>

                {/* Data Management */}
                <div className="space-y-4 pt-4 border-t border-zinc-200 dark:border-zinc-800">
                    <h3 className="text-xs font-semibold text-zinc-500 uppercase tracking-wider flex items-center gap-2">
                        <Trash2 size={12} />
                        Data Management
                    </h3>
                    <button
                        onClick={onClearChat}
                        className="w-full flex items-center justify-center gap-2 px-3 py-2 bg-red-50 dark:bg-red-900/10 hover:bg-red-100 dark:hover:bg-red-900/20 text-red-600 dark:text-red-400 border border-red-200 dark:border-red-900/30 rounded-md transition-colors text-xs font-medium"
                    >
                        <Trash2 size={14} />
                        Clear All Chat History
                    </button>
                </div>

                {/* L2 Memory - Knowledge Bases Status */}
                <div className="mt-4 p-4 bg-zinc-50 dark:bg-zinc-950/50 rounded border border-zinc-200 dark:border-zinc-800/50">
                <div className="flex items-center justify-between mb-3">
                    <h3 className="text-xs font-semibold text-zinc-500 uppercase flex items-center gap-2">
                        <Database size={12} />
                        L2 Memory (8 KBs)
                    </h3>
                    <button
                        onClick={fetchKbStatus}
                        disabled={kbLoading}
                        className="p-1 text-zinc-400 hover:text-zinc-600 dark:hover:text-zinc-300 transition-colors disabled:opacity-50"
                        title="Refresh KB Status"
                    >
                        <RefreshCw size={12} className={kbLoading ? 'animate-spin' : ''} />
                    </button>
                </div>
                
                {kbError && (
                    <div className="text-xs text-red-500 dark:text-red-400 mb-2 flex items-center gap-1">
                        <AlertCircle size={10} />
                        <span>{kbError}</span>
                    </div>
                )}
                
                {kbStatus && (
                    <>
                        <div className="flex items-center gap-2 mb-3 pb-2 border-b border-zinc-200 dark:border-zinc-800">
                            {kbStatus.all_connected ? (
                                <CheckCircle2 size={14} className="text-emerald-500" />
                            ) : (
                                <XCircle size={14} className="text-red-500" />
                            )}
                            <span className={`text-xs font-medium ${kbStatus.all_connected ? 'text-emerald-600 dark:text-emerald-400' : 'text-red-600 dark:text-red-400'}`}>
                                {kbStatus.all_connected ? 'All KBs Connected' : 'Some KBs Offline'}
                            </span>
                            <span className="text-[10px] text-zinc-400 ml-auto">
                                {kbStatus.total_entries} entries
                            </span>
                        </div>
                        
                        <div className="grid grid-cols-2 gap-1.5">
                            {kbStatus.knowledge_bases.map((kb) => (
                                <div 
                                    key={kb.slot_id}
                                    className={`flex items-center gap-1.5 px-2 py-1 rounded text-[10px] ${
                                        kb.connected 
                                            ? 'bg-emerald-50 dark:bg-emerald-900/20 text-emerald-700 dark:text-emerald-300' 
                                            : 'bg-red-50 dark:bg-red-900/20 text-red-700 dark:text-red-300'
                                    }`}
                                    title={`${kb.name}\nTree: ${kb.tree_name}\nEntries: ${kb.entry_count}${kb.error ? `\nError: ${kb.error}` : ''}`}
                                >
                                    <span className={`w-1.5 h-1.5 rounded-full ${kb.connected ? 'bg-emerald-500' : 'bg-red-500'}`} />
                                    <span className="font-medium truncate">KB-{kb.slot_id}</span>
                                    <span className="text-[9px] opacity-70 ml-auto">{kb.entry_count}</span>
                                </div>
                            ))}
                        </div>
                    </>
                )}
                
                {!kbStatus && !kbError && kbLoading && (
                    <div className="text-xs text-zinc-400 flex items-center gap-2">
                        <RefreshCw size={12} className="animate-spin" />
                        <span>Loading KB status...</span>
                    </div>
                )}
                </div>

                {/* Debug Info */}
                <div className="mt-4 p-4 bg-zinc-50 dark:bg-zinc-950/50 rounded border border-zinc-200 dark:border-zinc-800/50">
                <h3 className="text-xs font-semibold text-zinc-500 mb-2 uppercase">System Info</h3>
                <div className="text-xs text-zinc-500 dark:text-zinc-600 font-mono space-y-1">
                    <p>Status: <span className="text-orange-600 dark:text-orange-500">Ready</span></p>
                    <p>Memory: <span className="text-zinc-500">Bare Metal (Sled)</span></p>
                    <p>Protocol: <span className="text-zinc-500">REST/JSON</span></p>
                    <div className="pt-2 flex items-center gap-2 text-[10px] opacity-70">
                    <Terminal size={10} />
                    <span>{settings.llmModel} ({settings.llmTemperature})</span>
                    </div>
                </div>
                </div>
            </div>
        ) : (
            <div className="p-4 space-y-4 min-h-0 flex flex-col h-full">
                {/* Search & Filters */}
                <div className="space-y-3 pb-4 border-b border-zinc-200 dark:border-zinc-800 shrink-0">
                    <div className="relative">
                        <input 
                           type="text" 
                           placeholder="Search history..." 
                           value={searchQuery}
                           onChange={(e) => setSearchQuery(e.target.value)}
                           className="w-full bg-zinc-50 dark:bg-zinc-950 border border-zinc-200 dark:border-zinc-800 rounded-md pl-9 pr-3 py-2 text-sm text-zinc-900 dark:text-zinc-200 focus:outline-none focus:border-zinc-400 dark:focus:border-zinc-600"
                        />
                        <Search size={14} className="absolute left-3 top-1/2 -translate-y-1/2 text-zinc-400" />
                        {searchQuery && (
                            <button 
                                onClick={() => setSearchQuery('')}
                                className="absolute right-3 top-1/2 -translate-y-1/2 text-zinc-400 hover:text-zinc-600 dark:hover:text-zinc-300"
                            >
                                <X size={12} />
                            </button>
                        )}
                    </div>
                    
                    <div className="flex gap-2">
                        <div className="relative flex-1">
                           <select
                                value={roleFilter}
                                onChange={(e) => setRoleFilter(e.target.value as any)}
                                className="w-full appearance-none bg-zinc-50 dark:bg-zinc-950 border border-zinc-200 dark:border-zinc-800 rounded-md pl-8 pr-3 py-1.5 text-xs font-medium text-zinc-600 dark:text-zinc-400 focus:outline-none focus:border-zinc-400 dark:focus:border-zinc-600"
                            >
                                <option value="all">All Roles</option>
                                <option value="user">User Only</option>
                                <option value="agi">AGI Only</option>
                            </select>
                            <Filter size={12} className="absolute left-2.5 top-1/2 -translate-y-1/2 text-zinc-400" />
                        </div>
                         <div className="relative flex-1">
                           <select
                                value={timeFilter}
                                onChange={(e) => setTimeFilter(e.target.value as any)}
                                className="w-full appearance-none bg-zinc-50 dark:bg-zinc-950 border border-zinc-200 dark:border-zinc-800 rounded-md pl-8 pr-3 py-1.5 text-xs font-medium text-zinc-600 dark:text-zinc-400 focus:outline-none focus:border-zinc-400 dark:focus:border-zinc-600"
                            >
                                <option value="all">Any Time</option>
                                <option value="24h">Past 24h</option>
                                <option value="7d">Past 7 Days</option>
                            </select>
                            <Calendar size={12} className="absolute left-2.5 top-1/2 -translate-y-1/2 text-zinc-400" />
                        </div>
                    </div>
                </div>

                {/* Results List */}
                <div className="flex-1 overflow-y-auto min-h-0 -mx-2 px-2 space-y-3">
                    {filteredMessages.length === 0 ? (
                        <div className="flex flex-col items-center justify-center h-40 text-zinc-400 text-center">
                             <Search size={24} className="mb-2 opacity-20" />
                             <p className="text-xs">No messages found</p>
                        </div>
                    ) : (
                        filteredMessages.map(msg => (
                            <div key={msg.id} className="p-3 rounded-lg bg-zinc-50 dark:bg-zinc-950/50 border border-zinc-100 dark:border-zinc-800/50 hover:border-zinc-300 dark:hover:border-zinc-700 transition-colors group">
                                <div className="flex items-center justify-between mb-1.5">
                                    <span className={`text-[10px] font-bold uppercase tracking-wider px-1.5 py-0.5 rounded ${msg.role === 'user' ? 'bg-zinc-200 text-zinc-700 dark:bg-zinc-800 dark:text-zinc-300' : 'bg-indigo-100 text-indigo-700 dark:bg-indigo-900/30 dark:text-indigo-300'}`}>
                                        {msg.role}
                                    </span>
                                    <span className="text-[10px] text-zinc-400 font-mono">
                                        {new Date(msg.timestamp).toLocaleString(undefined, { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' })}
                                    </span>
                                </div>
                                <p className="text-xs text-zinc-600 dark:text-zinc-400 line-clamp-3 leading-relaxed font-medium">
                                    {msg.content}
                                </p>
                                {msg.thoughts && msg.thoughts.length > 0 && (
                                    <div className="mt-2 flex items-center gap-1.5 text-[10px] text-zinc-400 dark:text-zinc-500">
                                        <Brain size={10} />
                                        <span>{msg.thoughts.length} thought layers</span>
                                    </div>
                                )}
                            </div>
                        ))
                    )}
                </div>
            </div>
        )}
      </div>
    </div>
  );
};

export default SettingsSidebar;
