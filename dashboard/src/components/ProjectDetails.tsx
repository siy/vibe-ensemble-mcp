import { Show } from 'solid-js';
import type { Project } from '../api';

interface ProjectDetailsProps {
  project: Project;
}

function ProjectDetails(props: ProjectDetailsProps) {
  return (
    <article>
      <header>
        <h3>Project Details</h3>
      </header>

      <dl>
        <dt><strong>Name</strong></dt>
        <dd>{props.project.repository_name}</dd>

        <dt><strong>Prefix</strong></dt>
        <dd><code>{props.project.project_prefix}</code></dd>

        <dt><strong>Path</strong></dt>
        <dd><code>{props.project.path}</code></dd>

        <Show when={props.project.short_description}>
          <dt><strong>Description</strong></dt>
          <dd>{props.project.short_description}</dd>
        </Show>

        <dt><strong>Created</strong></dt>
        <dd>{new Date(props.project.created_at).toLocaleString()}</dd>

        <dt><strong>Updated</strong></dt>
        <dd>{new Date(props.project.updated_at).toLocaleString()}</dd>
      </dl>

      <Show when={props.project.jbct_enabled}>
        <details>
          <summary><strong>JBCT Configuration</strong></summary>
          <dl>
            <dt>Version</dt>
            <dd>{props.project.jbct_version || 'Unknown'}</dd>
            <Show when={props.project.jbct_url}>
              <dt>URL</dt>
              <dd>
                <a href={props.project.jbct_url!} target="_blank" rel="noopener noreferrer">
                  {props.project.jbct_url}
                </a>
              </dd>
            </Show>
          </dl>
        </details>
      </Show>

      <Show when={props.project.rules}>
        <details>
          <summary><strong>Project Rules</strong></summary>
          <pre style="overflow-x: auto; font-size: 0.85rem;">
            <code>{props.project.rules}</code>
          </pre>
        </details>
      </Show>

      <Show when={props.project.patterns}>
        <details>
          <summary><strong>Project Patterns</strong></summary>
          <pre style="overflow-x: auto; font-size: 0.85rem;">
            <code>{props.project.patterns}</code>
          </pre>
        </details>
      </Show>
    </article>
  );
}

export default ProjectDetails;
