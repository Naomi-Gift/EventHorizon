const batchService = require('../src/services/batch.service');

// Test batch service functionality
async function testBatchService() {
    console.log('Testing Batch Service...');

    // Mock trigger
    const mockTrigger = {
        _id: 'test-trigger-123',
        contractId: 'test-contract',
        eventName: 'test-event',
        actionType: 'webhook',
        batchingConfig: {
            enabled: true,
            windowMs: 2000, // 2 seconds for testing
            maxBatchSize: 3,
            continueOnError: true
        }
    };

    // Mock event payloads
    const event1 = { id: 1, data: 'event1' };
    const event2 = { id: 2, data: 'event2' };
    const event3 = { id: 3, data: 'event3' };
    const event4 = { id: 4, data: 'event4' };

    let batchesProcessed = [];

    // Mock flush callback
    const flushCallback = (events, trigger) => {
        console.log(`Flushing batch with ${events.length} events for trigger ${trigger._id}`);
        batchesProcessed.push({ events: [...events], triggerId: trigger._id });
    };

    // Add events
    console.log('Adding event 1...');
    batchService.addEvent(mockTrigger, event1, flushCallback);

    console.log('Adding event 2...');
    batchService.addEvent(mockTrigger, event2, flushCallback);

    console.log('Adding event 3 (should trigger flush due to maxBatchSize)...');
    batchService.addEvent(mockTrigger, event3, flushCallback);

    // Wait a bit and add another event
    setTimeout(() => {
        console.log('Adding event 4...');
        batchService.addEvent(mockTrigger, event4, flushCallback);

        // Wait for time-based flush
        setTimeout(() => {
            console.log('Test completed.');
            console.log('Batches processed:', batchesProcessed.length);
            batchesProcessed.forEach((batch, i) => {
                console.log(`Batch ${i + 1}: ${batch.events.length} events`);
            });

            // Check stats
            const stats = batchService.getStats();
            console.log('Final stats:', stats);

            process.exit(0);
        }, 2500);
    }, 500);
}

// Run test if this file is executed directly
if (require.main === module) {
    testBatchService();
}

module.exports = { testBatchService };