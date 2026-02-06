export interface Message {
  id: string;
  role: 'user' | 'agi';
  content: string;
  timestamp: number;
  thoughts?: ThoughtLayer[]; // For multi-layer memory/reasoning
  isError?: boolean;
  isPinned?: boolean;
}

export interface ThoughtLayer {
  id: string;
  title: string; // e.g., "Short-term Memory Retrieval", "Planner"
  content: string;
  expanded?: boolean;
}

export interface AppSettings {
  apiUrl: string;
  stream: boolean;
  showThoughts: boolean;
  userAlias?: string;
  userAvatar?: string;
  agiAvatar?: string;
  theme: 'dark' | 'light';
  customLogo?: string;
  customFavicon?: string;
  customCss?: string;
  
  // LLM Agent Settings
  llmModel: string;
  llmTemperature: number;
  llmMaxTokens: number;
  orchestratorPersona: string;
}

export interface ApiResponse {
  response: string;
  thoughts?: ThoughtLayer[];
  // Fallback for simple backends
  thought?: string; 
}