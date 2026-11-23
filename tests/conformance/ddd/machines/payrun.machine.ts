import { createMachine } from 'xstate';

/**
 * The PayRun lifecycle, authored in TypeScript/XState (issue pherolang#388)
 * — the reference-form twin of the inline machine in `payrun.ddd`. Guards
 * are serialized names, never inline functions; the states are the
 * literals of the bound `PayRunStatus` enum.
 */
export const payrunMachine = createMachine({
  id: 'payrun',
  initial: 'Draft',
  states: {
    Draft: {
      on: {
        CALCULATE: 'Calculating',
        REVERSE: 'Reversed',
      },
    },
    Calculating: {
      on: { SUBMIT: 'AwaitingApproval' },
    },
    AwaitingApproval: {
      on: { APPROVE: 'Approved' },
    },
    Approved: {
      on: { SCHEDULE: 'Processing' },
    },
    Processing: {
      on: { POST: 'Posted' },
    },
    Posted: { type: 'final' },
    Reversed: { type: 'final' },
  },
});
