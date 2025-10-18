import { For, Show, createSignal } from 'solid-js';
import type { Ticket } from '../api';
import TicketDetails from './TicketDetails';

interface TicketListProps {
  tickets: Ticket[];
  projectId: string;
  loading: boolean;
}

function TicketList(props: TicketListProps) {
  const [expandedTicketId, setExpandedTicketId] = createSignal<string | null>(null);

  function toggleTicket(ticketId: string) {
    setExpandedTicketId(expandedTicketId() === ticketId ? null : ticketId);
  }

  function getStateBadge(state: Ticket['state']) {
    const badges = {
      open: '🟢 Open',
      closed: '✅ Closed',
      on_hold: '⏸️  On Hold',
    };
    return badges[state] || state;
  }

  function getDependencyBadge(status: Ticket['dependency_status']) {
    const badges = {
      ready: '✅ Ready',
      blocked: '🚫 Blocked',
    };
    return badges[status] || status;
  }

  function getPriorityBadge(priority: Ticket['priority']) {
    const badges = {
      low: '🔵 Low',
      medium: '🟡 Medium',
      high: '🟠 High',
      urgent: '🔴 Urgent',
    };
    return badges[priority] || priority;
  }

  return (
    <article>
      <header>
        <h3>Tickets ({props.tickets.length})</h3>
      </header>

      <Show when={props.loading}>
        <p aria-busy="true">Loading tickets...</p>
      </Show>

      <Show when={!props.loading && props.tickets.length === 0}>
        <p>No tickets found for this project.</p>
      </Show>

      <Show when={!props.loading && props.tickets.length > 0}>
        <table>
          <thead>
            <tr>
              <th>ID</th>
              <th>Title</th>
              <th>Stage</th>
              <th>State</th>
              <th>Status</th>
              <th>Priority</th>
            </tr>
          </thead>
          <tbody>
            <For each={props.tickets}>
              {(ticket) => (
                <>
                  <tr
                    onClick={() => toggleTicket(ticket.ticket_id)}
                    style={{
                      cursor: 'pointer',
                      'background-color':
                        expandedTicketId() === ticket.ticket_id
                          ? 'var(--pico-table-row-selected-background-color, rgba(0,0,0,0.1))'
                          : undefined,
                    }}
                  >
                    <td>
                      <code style="font-size: 0.85rem;">{ticket.ticket_id}</code>
                    </td>
                    <td>{ticket.title}</td>
                    <td>
                      <code style="font-size: 0.85rem;">{ticket.current_stage}</code>
                    </td>
                    <td>{getStateBadge(ticket.state)}</td>
                    <td>{getDependencyBadge(ticket.dependency_status)}</td>
                    <td>{getPriorityBadge(ticket.priority)}</td>
                  </tr>
                  <Show when={expandedTicketId() === ticket.ticket_id}>
                    <tr>
                      <td colspan={6} style="padding: 0;">
                        <TicketDetails ticket={ticket} projectId={props.projectId} />
                      </td>
                    </tr>
                  </Show>
                </>
              )}
            </For>
          </tbody>
        </table>
      </Show>
    </article>
  );
}

export default TicketList;
