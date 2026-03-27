const test = require('node:test');
const assert = require('node:assert/strict');

const queue = require('../src/worker/queue');

const { actionQueue, enqueueAction, getQueueStats } = queue;
const originalAdd = actionQueue.add;
const originalGetWaitingCount = actionQueue.getWaitingCount;
const originalGetActiveCount = actionQueue.getActiveCount;
const originalGetCompletedCount = actionQueue.getCompletedCount;
const originalGetFailedCount = actionQueue.getFailedCount;
const originalGetDelayedCount = actionQueue.getDelayedCount;

test.after(() => {
    actionQueue.add = originalAdd;
    actionQueue.getWaitingCount = originalGetWaitingCount;
    actionQueue.getActiveCount = originalGetActiveCount;
    actionQueue.getCompletedCount = originalGetCompletedCount;
    actionQueue.getFailedCount = originalGetFailedCount;
    actionQueue.getDelayedCount = originalGetDelayedCount;
});

test('enqueueAction adds an action job with trigger and payload metadata', async () => {
    const trigger = {
        _id: 'test-trigger-123',
        actionType: 'webhook',
        actionUrl: 'https://example.com/webhook',
        contractId: 'CTEST123',
        eventName: 'transfer',
    };

    const eventPayload = {
        from: 'GTEST123',
        to: 'GTEST456',
        amount: '1000',
    };

    actionQueue.add = async (_name, data, options) => ({
        id: options.jobId,
        data,
    });

    const job = await enqueueAction(trigger, eventPayload);

    assert.ok(job);
    assert.match(job.id, /^test-trigger-123-\d+$/);
    assert.deepEqual(job.data.trigger, trigger);
    assert.deepEqual(job.data.eventPayload, eventPayload);
});

test('getQueueStats returns aggregated counts', async () => {
    actionQueue.getWaitingCount = async () => 1;
    actionQueue.getActiveCount = async () => 2;
    actionQueue.getCompletedCount = async () => 3;
    actionQueue.getFailedCount = async () => 4;
    actionQueue.getDelayedCount = async () => 5;

    const stats = await getQueueStats();

    assert.deepEqual(stats, {
        waiting: 1,
        active: 2,
        completed: 3,
        failed: 4,
        delayed: 5,
        total: 15,
    });
});
