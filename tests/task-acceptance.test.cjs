const test=require('node:test'), assert=require('node:assert/strict');
const {approvalPassed}=require('../scripts/task-acceptance.cjs');
test('real approval acceptance requires rejection, successful end and exactly one of each notification',()=>{
  const good={permissionRejected:true,states:['working','waiting_approval','ended'],notifications:['waiting_approval','ended']};
  assert.equal(approvalPassed(good),true);
  for(const patch of [
    {permissionRejected:false}, {states:['waiting_approval','failed']},
    {notifications:['waiting_approval']}, {notifications:['waiting_approval','ended','ended']},
    {notifications:['waiting_approval','waiting_approval','ended']}
  ]) assert.equal(approvalPassed({...good,...patch}),false);
});
