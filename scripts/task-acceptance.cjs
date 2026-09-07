// Shared pass criteria: a waiting notification alone is not a completed acceptance run.
exports.approvalPassed = ({states, notifications, permissionRejected}) =>
  permissionRejected === true && states.includes('waiting_approval') &&
  states.includes('ended') && !states.includes('failed') &&
  notifications.filter(x => x === 'waiting_approval').length === 1 &&
  notifications.filter(x => x === 'ended').length === 1;
