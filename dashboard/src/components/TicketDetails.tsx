import { createSignal, createMemo, onMount, Show, For } from 'solid-js';
import { fetchTicketWithComments, type Ticket, type Comment } from '../api';

interface TicketDetailsProps {
  ticket: Ticket;
  projectId: string;
}

function TicketDetails(props: TicketDetailsProps) {
  const [comments, setComments] = createSignal<Comment[]>([]);
  const [loading, setLoading] = createSignal(true);

  const executionPlan = createMemo(() => {
    try {
      return JSON.parse(props.ticket.execution_plan) as string[];
    } catch {
      return [];
    }
  });

  onMount(async () => {
    try {
      const data = await fetchTicketWithComments(props.projectId, props.ticket.ticket_id);
      setComments(data.comments);
    } catch (err) {
      console.error('Failed to load comments:', err);
    } finally {
      setLoading(false);
    }
  });

  return (
    <article style="margin: 1rem; background-color: var(--pico-background-color);">
      <header>
        <h4>Ticket Details</h4>
      </header>

      <dl>
        <dt><strong>Execution Plan</strong></dt>
        <dd>
          <For each={executionPlan()}>
            {(stage, index) => (
              <span>
                <code
                  style={{
                    'font-size': '0.85rem',
                    'background-color':
                      stage === props.ticket.current_stage
                        ? 'var(--pico-primary-background)'
                        : undefined,
                  }}
                >
                  {stage}
                </code>
                {index() < executionPlan().length - 1 && ' → '}
              </span>
            )}
          </For>
        </dd>

        <Show when={props.ticket.parent_ticket_id}>
          <dt><strong>Parent Ticket</strong></dt>
          <dd><code>{props.ticket.parent_ticket_id}</code></dd>
        </Show>

        <Show when={props.ticket.processing_worker_id}>
          <dt><strong>Processing Worker</strong></dt>
          <dd><code style="font-size: 0.85rem;">{props.ticket.processing_worker_id}</code></dd>
        </Show>

        <dt><strong>Created</strong></dt>
        <dd>{new Date(props.ticket.created_at).toLocaleString()}</dd>

        <dt><strong>Updated</strong></dt>
        <dd>{new Date(props.ticket.updated_at).toLocaleString()}</dd>

        <Show when={props.ticket.closed_at}>
          <dt><strong>Closed</strong></dt>
          <dd>{new Date(props.ticket.closed_at!).toLocaleString()}</dd>
        </Show>

        <Show when={props.ticket.resolution}>
          <dt><strong>Resolution</strong></dt>
          <dd>{props.ticket.resolution}</dd>
        </Show>
      </dl>

      <details open>
        <summary><strong>Comments ({comments().length})</strong></summary>

        <Show when={loading()}>
          <p aria-busy="true">Loading comments...</p>
        </Show>

        <Show when={!loading() && comments().length === 0}>
          <p>No comments yet.</p>
        </Show>

        <Show when={!loading() && comments().length > 0}>
          <div style="max-height: 400px; overflow-y: auto;">
            <For each={comments()}>
              {(comment) => (
                <article style="margin-bottom: 0.5rem; padding: 0.75rem;">
                  <header style="margin-bottom: 0.5rem;">
                    <small>
                      <strong>{comment.worker_type || 'system'}</strong>
                      {comment.worker_id && ` (${comment.worker_id})`}
                      {' • '}
                      {new Date(comment.created_at).toLocaleString()}
                      {comment.stage_index !== null && ` • Stage ${comment.stage_index}`}
                    </small>
                  </header>
                  <p style="white-space: pre-wrap; margin: 0; font-size: 0.9rem;">
                    {comment.content}
                  </p>
                </article>
              )}
            </For>
          </div>
        </Show>
      </details>
    </article>
  );
}

export default TicketDetails;
