import { createSignal, onMount, onCleanup, Show } from 'solid-js';
import { fetchProjects, fetchProject, fetchTickets, subscribeToEvents, type Project, type Ticket } from './api';
import ProjectSelector from './components/ProjectSelector';
import ProjectDetails from './components/ProjectDetails';
import TicketList from './components/TicketList';
import ThemeToggle from './components/ThemeToggle';

function App() {
  const [projects, setProjects] = createSignal<Project[]>([]);
  const [selectedProjectId, setSelectedProjectId] = createSignal<string | null>(null);
  const [selectedProject, setSelectedProject] = createSignal<Project | null>(null);
  const [tickets, setTickets] = createSignal<Ticket[]>([]);
  const [loading, setLoading] = createSignal(true);
  const [error, setError] = createSignal<string | null>(null);

  // Load projects on mount
  onMount(async () => {
    try {
      const projectList = await fetchProjects();
      setProjects(projectList);
      setLoading(false);

      // Auto-select first project if available
      if (projectList.length > 0) {
        selectProject(projectList[0].repository_name);
      }
    } catch (err) {
      setError((err as Error).message);
      setLoading(false);
    }

    // Subscribe to SSE events for real-time updates
    const unsubscribe = subscribeToEvents((event) => {
      try {
        const data = JSON.parse(event.data);
        console.log('SSE event:', data);

        // Reload tickets when relevant events occur
        if (selectedProjectId() && (data.event_type === 'ticket_created' ||
            data.event_type === 'ticket_updated' ||
            data.event_type === 'ticket_closed')) {
          loadTickets(selectedProjectId()!);
        }
      } catch (err) {
        console.error('Failed to parse SSE event:', err);
      }
    });

    onCleanup(unsubscribe);
  });

  async function selectProject(projectId: string) {
    setLoading(true);
    setError(null);
    setSelectedProjectId(projectId);

    try {
      const [project, projectTickets] = await Promise.all([
        fetchProject(projectId),
        fetchTickets(projectId),
      ]);

      setSelectedProject(project);
      setTickets(projectTickets);
    } catch (err) {
      setError((err as Error).message);
    } finally {
      setLoading(false);
    }
  }

  async function loadTickets(projectId: string) {
    try {
      const projectTickets = await fetchTickets(projectId);
      setTickets(projectTickets);
    } catch (err) {
      console.error('Failed to reload tickets:', err);
    }
  }

  return (
    <div class="container">
      <header>
        <hgroup>
          <h1>Vibe Ensemble Dashboard</h1>
          <p>Multi-agent coordination monitoring</p>
        </hgroup>
        <ThemeToggle />
      </header>

      <Show when={error()}>
        <article aria-label="Error">
          <p><strong>Error:</strong> {error()}</p>
        </article>
      </Show>

      <Show when={loading() && projects().length === 0}>
        <article aria-busy="true">Loading projects...</article>
      </Show>

      <Show when={!loading() && projects().length === 0 && !error()}>
        <article>
          <p>No projects found. Create a project using the MCP server to get started.</p>
        </article>
      </Show>

      <Show when={projects().length > 0}>
        <ProjectSelector
          projects={projects()}
          selectedProjectId={selectedProjectId()}
          onSelect={selectProject}
        />

        <Show when={selectedProject()}>
          <ProjectDetails project={selectedProject()!} />

          <TicketList
            tickets={tickets()}
            projectId={selectedProjectId()!}
            loading={loading()}
          />
        </Show>
      </Show>

      <footer>
        <small>
          Vibe Ensemble MCP v1.0.0 | <a href="https://github.com/siy/vibe-ensemble-mcp" target="_blank">GitHub</a>
        </small>
      </footer>
    </div>
  );
}

export default App;
