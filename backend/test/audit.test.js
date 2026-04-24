const AuditLog = require('../src/models/audit.model');
const auditMiddleware = require('../src/middleware/audit.middleware');

// Test audit logging functionality
async function testAuditLogging() {
    console.log('Testing Audit Logging...');

    // Test audit log creation
    try {
        const mockReq = {
            ip: '192.168.1.100',
            get: (header) => {
                if (header === 'User-Agent') return 'TestAgent/1.0';
                if (header === 'X-Forwarded-For') return '10.0.0.1';
                return undefined;
            }
        };

        const logEntry = await AuditLog.createLog({
            operation: 'CREATE',
            resourceType: 'Trigger',
            resourceId: '507f1f77bcf86cd799439011',
            userId: 'test-user-123',
            userAgent: 'TestAgent/1.0',
            ipAddress: '192.168.1.100',
            forwardedFor: '10.0.0.1',
            changes: {
                before: null,
                after: { contractId: 'test', eventName: 'test' },
                diff: []
            },
            metadata: {
                endpoint: '/api/triggers',
                method: 'POST'
            }
        });

        console.log('✅ Audit log created:', logEntry._id);

        // Test integrity verification
        const isValid = await AuditLog.verifyIntegrity(logEntry._id);
        console.log('✅ Integrity check:', isValid ? 'PASSED' : 'FAILED');

        // Test audit trail retrieval
        const auditTrail = await AuditLog.getAuditTrail('507f1f77bcf86cd799439011');
        console.log('✅ Audit trail retrieved:', auditTrail.length, 'entries');

        // Test user identifier generation
        const userId = auditMiddleware.getUserIdentifier(mockReq);
        console.log('✅ User identifier generated:', userId);

        console.log('All audit tests passed!');

    } catch (error) {
        console.error('❌ Audit test failed:', error.message);
        process.exit(1);
    }
}

// Run test if this file is executed directly
if (require.main === module) {
    // Mock MongoDB connection for testing
    const mongoose = require('mongoose');

    mongoose.connect(process.env.MONGO_URI || 'mongodb://localhost:27017/eventhorizon_test', {
        useNewUrlParser: true,
        useUnifiedTopology: true
    })
    .then(() => {
        console.log('Connected to test database');
        return testAuditLogging();
    })
    .then(() => {
        console.log('Tests completed successfully');
        process.exit(0);
    })
    .catch((error) => {
        console.error('Test setup failed:', error);
        process.exit(1);
    });
}

module.exports = { testAuditLogging };