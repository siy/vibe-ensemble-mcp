export interface Project {
  repository_name: string;
  project_prefix: string;
  path: string;
  short_description: string | null;
  created_at: string;
  updated_at: string;
  rules: string | null;
  patterns: string | null;
  rules_version: number | null;
  patterns_version: number | null;
  jbct_enabled: boolean;
  jbct_version: string | null;
  jbct_url: string | null;
}

export interface Ticket {
  ticket_id: string;
  project_id: string;
  parent_ticket_id: string | null;
  title: string;
  execution_plan: string; // JSON string from database
  current_stage: string;
  state: 'open' | 'closed' | 'on_hold';
  priority: 'low' | 'medium' | 'high' | 'urgent';
  dependency_status: 'ready' | 'blocked';
  processing_worker_id: string | null;
  created_at: string;
  updated_at: string;
  closed_at: string | null;
  resolution: string | null;
}

export interface Comment {
  id: number;
  ticket_id: string;
  worker_type: string | null;
  worker_id: string | null;
  stage_index: number | null;
  content: string;
  created_at: string;
}

export interface TicketWithComments {
  ticket: Ticket;
  comments: Comment[];
}

const API_BASE = '/api';

export async function fetchProjects(): Promise<Project[]> {
  const response = await fetch(`${API_BASE}/projects`);
  if (!response.ok) {
    throw new Error(`Failed to fetch projects: ${response.statusText}`);
  }
  return response.json();
}

export async function fetchProject(projectId: string): Promise<Project> {
  const response = await fetch(`${API_BASE}/projects/${encodeURIComponent(projectId)}`);
  if (!response.ok) {
    throw new Error(`Failed to fetch project: ${response.statusText}`);
  }
  return response.json();
}

export async function fetchTickets(projectId: string): Promise<Ticket[]> {
  const response = await fetch(`${API_BASE}/projects/${encodeURIComponent(projectId)}/tickets`);
  if (!response.ok) {
    throw new Error(`Failed to fetch tickets: ${response.statusText}`);
  }
  return response.json();
}

export async function fetchTicketWithComments(
  projectId: string,
  ticketId: string
): Promise<TicketWithComments> {
  const response = await fetch(
    `${API_BASE}/projects/${encodeURIComponent(projectId)}/tickets/${encodeURIComponent(ticketId)}`
  );
  if (!response.ok) {
    throw new Error(`Failed to fetch ticket: ${response.statusText}`);
  }
  return response.json();
}

export function subscribeToEvents(callback: (event: MessageEvent) => void): () => void {
  const eventSource = new EventSource('/sse');

  eventSource.onmessage = callback;

  eventSource.onerror = (error) => {
    console.error('SSE connection error:', error);
  };

  return () => eventSource.close();
}
