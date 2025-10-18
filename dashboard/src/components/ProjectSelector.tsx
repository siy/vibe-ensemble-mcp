import { For } from 'solid-js';
import type { Project } from '../api';

interface ProjectSelectorProps {
  projects: Project[];
  selectedProjectId: string | null;
  onSelect: (projectId: string) => void;
}

function ProjectSelector(props: ProjectSelectorProps) {
  return (
    <article>
      <header>
        <h3>Select Project</h3>
      </header>
      <select
        value={props.selectedProjectId || ''}
        onChange={(e) => props.onSelect(e.currentTarget.value)}
      >
        <option value="" disabled>
          Choose a project...
        </option>
        <For each={props.projects}>
          {(project) => (
            <option value={project.repository_name}>
              {project.repository_name}
              {project.short_description ? ` - ${project.short_description}` : ''}
            </option>
          )}
        </For>
      </select>
    </article>
  );
}

export default ProjectSelector;
