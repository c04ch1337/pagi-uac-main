import React, { useState, useEffect } from 'react';
import { Settings, Boxes, Pin, Plus } from 'lucide-react';
import ChatInterface from './components/ChatInterface';
import SettingsSidebar from './components/SettingsSidebar';
import PinnedSidebar from './components/PinnedSidebar';
import { Message, AppSettings, ApiResponse } from './types';
import { sendMessageToOrchestrator, streamMessageToOrchestrator } from './services/apiService';

const App: React.FC = () => {
  const [isSidebarOpen, setIsSidebarOpen] = useState(false);
  const [isPinnedSidebarOpen, setIsPinnedSidebarOpen] = useState(false);
  const [isLoading, setIsLoading] = useState(false);
  const [isStreaming, setIsStreaming] = useState(false);
  
  // Initialize settings from localStorage if available
  // Sovereign architecture: Gateway is hard-locked to port 8001. UI must point only to 8001.
  const GATEWAY_API_URL = 'http://127.0.0.1:8001/api/v1/chat';

  const [settings, setSettings] = useState<AppSettings>(() => {
    const savedSettings = localStorage.getItem('agi_settings');
    if (savedSettings) {
      try {
        const parsed = JSON.parse(savedSettings);
        // Ensure theme exists for migration
        if (!parsed.theme) parsed.theme = 'dark';
        // Ensure userAlias exists for migration
        if (!parsed.userAlias) parsed.userAlias = 'User';
        // Ensure LLM settings exist for migration
        if (!parsed.llmModel) parsed.llmModel = 'openai/gpt-4o-mini';
        // Migration: replace invalid OpenRouter model IDs (old UI used llama3-70b-8192 which is not valid).
        if (parsed.llmModel === 'llama3-70b-8192') parsed.llmModel = 'meta-llama/llama-3.3-70b-instruct:free';
        if (parsed.llmTemperature === undefined) parsed.llmTemperature = 0.7;
        if (!parsed.llmMaxTokens) parsed.llmMaxTokens = 8192;
        if (!parsed.orchestratorPersona) parsed.orchestratorPersona = 'general_assistant';

        // Enforce port 8001 only (any other host/port is overwritten)
        parsed.apiUrl = GATEWAY_API_URL;

        return parsed;
      } catch (e) {
        console.error("Failed to parse settings", e);
      }
    }
    return {
      apiUrl: GATEWAY_API_URL,
      stream: true,
      showThoughts: true,
      userAlias: 'User',
      theme: 'dark',
      llmModel: 'openai/gpt-4o-mini',
      llmTemperature: 0.7,
      llmMaxTokens: 8192,
      orchestratorPersona: 'general_assistant',
    };
  });

  // Save settings to localStorage whenever they change
  useEffect(() => {
    try {
      localStorage.setItem('agi_settings', JSON.stringify(settings));
    } catch (e) {
      console.error("Failed to save settings to localStorage", e);
    }
  }, [settings]);

  // Apply Theme
  useEffect(() => {
    if (settings.theme === 'dark') {
      document.documentElement.classList.add('dark');
    } else {
      document.documentElement.classList.remove('dark');
    }
  }, [settings.theme]);

  // Effect to handle favicon updates (Robust Handler)
  useEffect(() => {
    const updateFavicon = (url: string) => {
      // Remove any existing favicon links to prevent conflicts
      const existingLinks = document.querySelectorAll("link[rel*='icon']");
      existingLinks.forEach(link => link.remove());
      
      // Create and append the new favicon link
      const link = document.createElement('link');
      link.type = 'image/x-icon';
      link.rel = 'shortcut icon';
      link.href = url;
      document.head.appendChild(link);
    };

    const defaultFavicon = 'data:image/svg+xml,<svg xmlns=%22http://www.w3.org/2000/svg%22 viewBox=%220 0 100 100%22><text y=%22.9em%22 font-size=%2290%22>🤖</text></svg>';
    updateFavicon(settings.customFavicon || defaultFavicon);
  }, [settings.customFavicon]);

  // Global Keyboard Shortcuts
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      const isModifier = e.metaKey || e.ctrlKey;

      // Toggle Settings: Ctrl+S / Cmd+S
      if (isModifier && (e.key === 's' || e.key === 'S')) {
        e.preventDefault(); // Prevent Save dialog
        setIsSidebarOpen(prev => !prev);
        // Ensure other sidebar is closed if we're opening this one
        if (!isSidebarOpen) setIsPinnedSidebarOpen(false);
      }

      // Toggle Pinned: Ctrl+P / Cmd+P
      if (isModifier && (e.key === 'p' || e.key === 'P')) {
        e.preventDefault(); // Prevent Print dialog
        setIsPinnedSidebarOpen(prev => !prev);
        // Ensure other sidebar is closed if we're opening this one
        if (!isPinnedSidebarOpen) setIsSidebarOpen(false);
      }

      // Close Sidebars: Escape
      if (e.key === 'Escape') {
        setIsSidebarOpen(false);
        setIsPinnedSidebarOpen(false);
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [isSidebarOpen, isPinnedSidebarOpen]);

  // Initialize messages from localStorage or default
  const [messages, setMessages] = useState<Message[]>(() => {
    const savedHistory = localStorage.getItem('agi_chat_history');
    if (savedHistory) {
      try {
        return JSON.parse(savedHistory);
      } catch (e) {
        console.error("Failed to parse chat history", e);
      }
    }
    return [];
  });

  // Save messages to localStorage whenever they change
  useEffect(() => {
    try {
      localStorage.setItem('agi_chat_history', JSON.stringify(messages));
    } catch (e) {
      console.error("Failed to save chat history", e);
    }
  }, [messages]);

  const togglePin = (messageId: string) => {
    setMessages(prev => prev.map(msg => 
      msg.id === messageId ? { ...msg, isPinned: !msg.isPinned } : msg
    ));
  };

  const handleClearChat = () => {
    if (window.confirm("Are you sure you want to start a new chat? This will clear the current history.")) {
      setMessages([]);
      try {
        localStorage.removeItem('agi_chat_history');
      } catch (e) {
        console.error("Failed to clear chat history from localStorage", e);
      }
    }
  };

  const handleSendMessage = async (text: string) => {
    const userMsg: Message = {
      id: Date.now().toString(),
      role: 'user',
      content: text,
      timestamp: Date.now(),
    };

    setMessages(prev => [...prev, userMsg]);
    setIsLoading(true);
    setIsStreaming(true);

    try {
      if (settings.stream) {
        const agiMsgId = (Date.now() + 1).toString();
        let accumulatedResponse = '';
        let hasCreatedMessage = false;

        const stream = streamMessageToOrchestrator(text, settings);

        for await (const chunk of stream) {
          accumulatedResponse += chunk;

          if (!hasCreatedMessage) {
            setIsLoading(false);
            const agiMsg: Message = {
              id: agiMsgId,
              role: 'agi',
              content: accumulatedResponse,
              timestamp: Date.now(),
            };
            setMessages(prev => [...prev, agiMsg]);
            hasCreatedMessage = true;
          } else {
            setMessages(prev => 
              prev.map(msg => 
                msg.id === agiMsgId 
                  ? { ...msg, content: accumulatedResponse }
                  : msg
              )
            );
          }
        }
        
        setIsStreaming(false);

        // If stream finished but no content was received (rare)
        if (!hasCreatedMessage) {
           setIsLoading(false);
        }

      } else {
        const data: ApiResponse = await sendMessageToOrchestrator(text, settings);

        const agiMsg: Message = {
          id: (Date.now() + 1).toString(),
          role: 'agi',
          content: data.response,
          thoughts: data.thoughts,
          timestamp: Date.now(),
        };

        setMessages(prev => [...prev, agiMsg]);
        setIsLoading(false);
        setIsStreaming(false);
      }
    } catch (error) {
      console.error(error);
      const errorMsg: Message = {
        id: (Date.now() + 1).toString(),
        role: 'agi',
        content: `Connection Error: Failed to reach ${settings.apiUrl}. Ensure your Rust backend is running.`,
        isError: true,
        timestamp: Date.now(),
      };
      setMessages(prev => [...prev, errorMsg]);
      setIsLoading(false);
      setIsStreaming(false);
    }
  };

  return (
    <>
      {settings.customCss && (
        <style dangerouslySetInnerHTML={{ __html: settings.customCss }} />
      )}
      <div className="h-screen w-full bg-zinc-50 dark:bg-zinc-950 text-zinc-900 dark:text-zinc-200 flex flex-col font-sans overflow-hidden transition-colors duration-300">
        {/* Header */}
        <header className="h-14 border-b border-zinc-200 dark:border-zinc-800 flex items-center justify-between px-6 bg-white/80 dark:bg-zinc-950/80 backdrop-blur-sm z-20 flex-shrink-0 transition-colors duration-300">
          <div className="flex items-center gap-3">
            {settings.customLogo ? (
              <img 
                src={settings.customLogo} 
                alt="Logo" 
                className="h-8 w-auto object-contain rounded-sm" 
              />
            ) : (
              <div className="bg-zinc-100 dark:bg-zinc-800 p-1.5 rounded text-orange-500 dark:text-orange-400">
                <Boxes size={18} />
              </div>
            )}
            <span className="font-semibold text-sm tracking-wide text-zinc-900 dark:text-zinc-100">PAGI ORCHESTRATOR</span>
            <span className="text-xs text-zinc-500 dark:text-zinc-600 bg-zinc-100 dark:bg-zinc-900 border border-zinc-200 dark:border-zinc-800 px-2 py-0.5 rounded-full">v0.1.0-alpha</span>
          </div>
          
          <div className="flex items-center gap-2">
            <button 
              onClick={handleClearChat}
              className="p-2 text-zinc-500 hover:text-zinc-900 dark:text-zinc-500 dark:hover:text-zinc-200 hover:bg-zinc-100 dark:hover:bg-zinc-900 rounded-md transition-all"
              title="New Chat"
            >
              <Plus size={20} />
            </button>
             <button 
              onClick={() => setIsPinnedSidebarOpen(true)}
              className="p-2 text-zinc-500 hover:text-zinc-900 dark:text-zinc-500 dark:hover:text-zinc-200 hover:bg-zinc-100 dark:hover:bg-zinc-900 rounded-md transition-all relative group"
              title="Pinned Messages (Ctrl+P)"
            >
              <Pin size={20} />
              {/* Optional: Indicator dot if pins exist */}
              {messages.some(m => m.isPinned) && (
                 <span className="absolute top-2 right-2 w-2 h-2 bg-orange-500 rounded-full border border-white dark:border-zinc-950"></span>
              )}
            </button>
            <button 
              onClick={() => setIsSidebarOpen(true)}
              className="p-2 text-zinc-500 hover:text-zinc-900 dark:text-zinc-500 dark:hover:text-zinc-200 hover:bg-zinc-100 dark:hover:bg-zinc-900 rounded-md transition-all"
              title="Settings (Ctrl+S)"
            >
              <Settings size={20} />
            </button>
          </div>
        </header>

        {/* Main Content */}
        <main className="flex-1 overflow-hidden relative">
          <ChatInterface 
            messages={messages} 
            isLoading={isLoading} 
            isStreaming={isStreaming}
            onSendMessage={handleSendMessage}
            settings={settings}
            onTogglePin={togglePin}
          />
          
          {/* Overlay for Sidebars */}
          {(isSidebarOpen || isPinnedSidebarOpen) && (
            <div 
              className="absolute inset-0 bg-black/20 dark:bg-black/50 backdrop-blur-sm z-40 transition-opacity"
              onClick={() => {
                setIsSidebarOpen(false);
                setIsPinnedSidebarOpen(false);
              }}
            />
          )}
          
          <SettingsSidebar 
            isOpen={isSidebarOpen} 
            onClose={() => setIsSidebarOpen(false)}
            settings={settings}
            setSettings={setSettings}
            messages={messages}
            onClearChat={handleClearChat}
          />

          <PinnedSidebar
             isOpen={isPinnedSidebarOpen}
             onClose={() => setIsPinnedSidebarOpen(false)}
             messages={messages}
             onTogglePin={togglePin}
          />
        </main>
      </div>
    </>
  );
};

export default App;
