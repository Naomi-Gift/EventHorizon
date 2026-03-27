const { Queue } = require('bullmq');
const Redis = require('ioredis');
const logger = require('../config/logger');

const REDIS_HOST = process.env.REDIS_HOST || 'localhost';
const REDIS_PORT = process.env.REDIS_PORT || 6379;
const REDIS_PASSWORD = process.env.REDIS_PASSWORD || undefined;

const connection = new Redis({
    host: REDIS_HOST,
    port: REDIS_PORT,
    password: REDIS_PASSWORD,
    lazyConnect: true,
    maxRetriesPerRequest: null,
});

let queueInstance;

function getQueue() {
    if (!queueInstance) {
        queueInstance = new Queue('action-queue', {
            connection,
            defaultJobOptions: {
                attempts: 3,
                backoff: {
                    type: 'exponential',
                    delay: 2000,
                },
                removeOnComplete: {
                    age: 86400, // Keep completed jobs for 24 hours
                    count: 1000,
                },
                removeOnFail: {
                    age: 604800, // Keep failed jobs for 7 days
                },
            },
        });
    }

    return queueInstance;
}

const actionQueue = {
    add(...args) {
        return getQueue().add(...args);
    },
    getWaitingCount(...args) {
        return getQueue().getWaitingCount(...args);
    },
    getActiveCount(...args) {
        return getQueue().getActiveCount(...args);
    },
    getCompletedCount(...args) {
        return getQueue().getCompletedCount(...args);
    },
    getFailedCount(...args) {
        return getQueue().getFailedCount(...args);
    },
    getDelayedCount(...args) {
        return getQueue().getDelayedCount(...args);
    },
    clean(...args) {
        return getQueue().clean(...args);
    },
    getWaiting(...args) {
        return getQueue().getWaiting(...args);
    },
    getActive(...args) {
        return getQueue().getActive(...args);
    },
    getCompleted(...args) {
        return getQueue().getCompleted(...args);
    },
    getFailed(...args) {
        return getQueue().getFailed(...args);
    },
    getDelayed(...args) {
        return getQueue().getDelayed(...args);
    },
    getJob(...args) {
        return getQueue().getJob(...args);
    },
};

/**
 * Add an action job to the queue
 * @param {Object} trigger - The trigger configuration
 * @param {Object} eventPayload - The event data
 * @returns {Promise<Job>} The created job
 */
async function enqueueAction(trigger, eventPayload) {
    try {
        const job = await actionQueue.add(
            `${trigger.actionType}-${trigger.contractId}`,
            {
                trigger,
                eventPayload,
            },
            {
                priority: trigger.priority || 1,
                jobId: `${trigger._id}-${Date.now()}`,
            }
        );

        logger.info('Action enqueued', {
            jobId: job.id,
            actionType: trigger.actionType,
            contractId: trigger.contractId,
            eventName: trigger.eventName,
        });

        return job;
    } catch (error) {
        logger.error('Failed to enqueue action', {
            actionType: trigger.actionType,
            contractId: trigger.contractId,
            error: error.message,
        });
        throw error;
    }
}

/**
 * Get queue statistics
 */
async function getQueueStats() {
    const [waiting, active, completed, failed, delayed] = await Promise.all([
        actionQueue.getWaitingCount(),
        actionQueue.getActiveCount(),
        actionQueue.getCompletedCount(),
        actionQueue.getFailedCount(),
        actionQueue.getDelayedCount(),
    ]);

    return {
        waiting,
        active,
        completed,
        failed,
        delayed,
        total: waiting + active + completed + failed + delayed,
    };
}

/**
 * Clean old jobs from the queue
 */
async function cleanQueue() {
    try {
        await actionQueue.clean(86400000, 1000, 'completed'); // 24 hours
        await actionQueue.clean(604800000, 1000, 'failed'); // 7 days
        logger.info('Queue cleaned successfully');
    } catch (error) {
        logger.error('Failed to clean queue', { error: error.message });
    }
}

module.exports = {
    actionQueue,
    enqueueAction,
    getQueueStats,
    cleanQueue,
};
