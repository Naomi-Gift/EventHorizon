const logger = require('../config/logger');

/**
 * Batch collector for high-frequency events
 * Handles window-based batching with configurable time windows and max batch sizes
 */
class BatchCollector {
    constructor() {
        this.batches = new Map(); // triggerId -> batch data
        this.timers = new Map(); // triggerId -> timeout handle
    }

    /**
     * Add an event to a batch for the given trigger
     * @param {Object} trigger - The trigger configuration
     * @param {Object} eventPayload - The event data
     * @param {Function} flushCallback - Callback to execute when batch is ready
     */
    addEvent(trigger, eventPayload, flushCallback) {
        const triggerId = trigger._id.toString();
        const batchConfig = trigger.batchingConfig || {};

        if (!batchConfig.enabled) {
            // If batching is disabled, execute immediately
            flushCallback([eventPayload], trigger);
            return;
        }

        // Initialize batch if it doesn't exist
        if (!this.batches.has(triggerId)) {
            this.batches.set(triggerId, {
                trigger,
                events: [],
                createdAt: Date.now(),
                flushCallback
            });
        }

        const batch = this.batches.get(triggerId);
        batch.events.push(eventPayload);

        // Check if batch should be flushed due to size limit
        if (batch.events.length >= batchConfig.maxBatchSize) {
            this.flushBatch(triggerId);
            return;
        }

        // Set up or reset the time window timer
        this.scheduleFlush(triggerId, batchConfig.windowMs);
    }

    /**
     * Schedule a batch flush after the specified delay
     * @param {string} triggerId - The trigger ID
     * @param {number} delayMs - Delay in milliseconds
     */
    scheduleFlush(triggerId, delayMs) {
        // Clear existing timer
        if (this.timers.has(triggerId)) {
            clearTimeout(this.timers.get(triggerId));
        }

        // Set new timer
        const timer = setTimeout(() => {
            this.flushBatch(triggerId);
        }, delayMs);

        this.timers.set(triggerId, timer);
    }

    /**
     * Flush a batch and execute the callback
     * @param {string} triggerId - The trigger ID
     */
    flushBatch(triggerId) {
        const batch = this.batches.get(triggerId);
        if (!batch || batch.events.length === 0) {
            return;
        }

        // Clear timer
        if (this.timers.has(triggerId)) {
            clearTimeout(this.timers.get(triggerId));
            this.timers.delete(triggerId);
        }

        // Execute callback with the batch
        try {
            batch.flushCallback(batch.events, batch.trigger);
        } catch (error) {
            logger.error('Error in batch flush callback', {
                triggerId,
                error: error.message,
                batchSize: batch.events.length
            });
        }

        // Remove the batch
        this.batches.delete(triggerId);

        logger.info('Batch flushed', {
            triggerId,
            batchSize: batch.events.length,
            batchAge: Date.now() - batch.createdAt
        });
    }

    /**
     * Force flush all pending batches (useful for graceful shutdown)
     */
    flushAll() {
        logger.info('Flushing all pending batches');

        for (const triggerId of this.batches.keys()) {
            this.flushBatch(triggerId);
        }

        // Clear all timers
        for (const timer of this.timers.values()) {
            clearTimeout(timer);
        }
        this.timers.clear();
    }

    /**
     * Get current batch statistics
     */
    getStats() {
        const stats = {
            activeBatches: this.batches.size,
            pendingTimers: this.timers.size,
            batches: []
        };

        for (const [triggerId, batch] of this.batches) {
            stats.batches.push({
                triggerId,
                eventCount: batch.events.length,
                age: Date.now() - batch.createdAt,
                contractId: batch.trigger.contractId,
                eventName: batch.trigger.eventName
            });
        }

        return stats;
    }
}

// Export singleton instance
module.exports = new BatchCollector();