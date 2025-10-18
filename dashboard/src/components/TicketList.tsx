import { For, Show, createSignal, createMemo } from 'solid-js';
import type { Ticket } from '../api';
import TicketDetails from './TicketDetails';

interface TicketListProps {
  tickets: Ticket[];
  projectId: string;
  loading: boolean;
}

type SortColumn = 'id' | 'title' | 'stage' | 'state' | 'created';
type SortDirection = 'asc' | 'desc';

function TicketList(props: TicketListProps) {
  const [expandedTicketId, setExpandedTicketId] = createSignal<string | null>(null);
  const [sortColumn, setSortColumn] = createSignal<SortColumn>('created');
  const [sortDirection, setSortDirection] = createSignal<SortDirection>('desc');

  function toggleTicket(ticketId: string) {
    setExpandedTicketId(expandedTicketId() === ticketId ? null : ticketId);
  }

  function handleSort(column: SortColumn) {
    if (sortColumn() === column) {
      setSortDirection(sortDirection() === 'asc' ? 'desc' : 'asc');
    } else {
      setSortColumn(column);
      setSortDirection('asc');
    }
  }

  const sortedTickets = createMemo(() => {
    const tickets = [...props.tickets];
    const direction = sortDirection() === 'asc' ? 1 : -1;

    return tickets.sort((a, b) => {
      let comparison = 0;

      switch (sortColumn()) {
        case 'id':
          comparison = a.ticket_id.localeCompare(b.ticket_id);
          break;
        case 'title':
          comparison = a.title.localeCompare(b.title);
          break;
        case 'stage':
          comparison = a.current_stage.localeCompare(b.current_stage);
          break;
        case 'state':
          comparison = a.state.localeCompare(b.state);
          break;
        case 'created':
          comparison = new Date(a.created_at).getTime() - new Date(b.created_at).getTime();
          break;
      }

      return comparison * direction;
    });
  });

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
              <th
                onClick={() => handleSort('id')}
                style={{ cursor: 'pointer', 'user-select': 'none' }}
              >
                ID {sortColumn() === 'id' && (sortDirection() === 'asc' ? '▲' : '▼')}
              </th>
              <th
                onClick={() => handleSort('title')}
                style={{ cursor: 'pointer', 'user-select': 'none' }}
              >
                Title {sortColumn() === 'title' && (sortDirection() === 'asc' ? '▲' : '▼')}
              </th>
              <th
                onClick={() => handleSort('stage')}
                style={{ cursor: 'pointer', 'user-select': 'none' }}
              >
                Stage {sortColumn() === 'stage' && (sortDirection() === 'asc' ? '▲' : '▼')}
              </th>
              <th
                onClick={() => handleSort('state')}
                style={{ cursor: 'pointer', 'user-select': 'none' }}
              >
                State {sortColumn() === 'state' && (sortDirection() === 'asc' ? '▲' : '▼')}
              </th>
              <th
                onClick={() => handleSort('created')}
                style={{ cursor: 'pointer', 'user-select': 'none' }}
              >
                Created {sortColumn() === 'created' && (sortDirection() === 'asc' ? '▲' : '▼')}
              </th>
              <th>Priority</th>
            </tr>
          </thead>
          <tbody>
            <For each={sortedTickets()}>
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
                    <td>
                      <small>{new Date(ticket.created_at).toLocaleString()}</small>
                    </td>
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
