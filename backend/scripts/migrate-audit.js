const mongoose = require('mongoose');
require('dotenv').config();

/**
 * Migration script to set up audit logging indexes and constraints
 * Run this once after deploying the audit logging feature
 */

async function runAuditMigration() {
    try {
        console.log('Starting audit logging migration...');

        // Connect to MongoDB
        await mongoose.connect(process.env.MONGO_URI, {
            useNewUrlParser: true,
            useUnifiedTopology: true
        });

        const db = mongoose.connection.db;
        const collection = db.collection('audit_logs');

        console.log('Creating indexes for audit_logs collection...');

        // Create indexes for efficient querying
        const indexes = [
            // Primary query patterns
            { key: { resourceId: 1, timestamp: -1 }, name: 'resourceId_timestamp' },
            { key: { operation: 1, timestamp: -1 }, name: 'operation_timestamp' },
            { key: { ipAddress: 1, timestamp: -1 }, name: 'ipAddress_timestamp' },
            { key: { userId: 1, timestamp: -1 }, name: 'userId_timestamp' },

            // Integrity verification
            { key: { integrityHash: 1 }, name: 'integrityHash', unique: true },

            // Analytics and reporting
            { key: { timestamp: -1 }, name: 'timestamp_desc' },
            { key: { 'metadata.endpoint': 1, timestamp: -1 }, name: 'endpoint_timestamp' },

            // Compound indexes for complex queries
            { key: { operation: 1, resourceType: 1, timestamp: -1 }, name: 'operation_resourceType_timestamp' }
        ];

        for (const indexSpec of indexes) {
            try {
                const result = await collection.createIndex(indexSpec.key, {
                    name: indexSpec.name,
                    unique: indexSpec.unique || false,
                    background: true // Don't block other operations
                });
                console.log(`✅ Created index: ${indexSpec.name}`);
            } catch (error) {
                if (error.code === 85) {
                    console.log(`ℹ️  Index ${indexSpec.name} already exists`);
                } else {
                    console.error(`❌ Failed to create index ${indexSpec.name}:`, error.message);
                }
            }
        }

        // Create a partial index for large change objects (only when changes exist)
        try {
            await collection.createIndex(
                { 'changes.diff': 1 },
                {
                    name: 'changes_diff_partial',
                    partialFilterExpression: { 'changes.diff': { $exists: true, $ne: [] } },
                    background: true
                }
            );
            console.log('✅ Created partial index for changes.diff');
        } catch (error) {
            console.log('ℹ️  Partial index for changes.diff already exists or not needed');
        }

        // Set up collection validation (optional - for additional data integrity)
        try {
            await db.command({
                collMod: 'audit_logs',
                validator: {
                    $jsonSchema: {
                        bsonType: 'object',
                        required: ['operation', 'resourceType', 'resourceId', 'ipAddress', 'timestamp'],
                        properties: {
                            operation: {
                                enum: ['CREATE', 'UPDATE', 'DELETE']
                            },
                            resourceType: {
                                type: 'string'
                            },
                            resourceId: {
                                type: 'objectId'
                            },
                            ipAddress: {
                                type: 'string'
                            },
                            timestamp: {
                                type: 'date'
                            },
                            integrityHash: {
                                type: 'string'
                            }
                        }
                    }
                },
                validationLevel: 'moderate' // Allow invalid documents but log warnings
            });
            console.log('✅ Collection validation enabled');
        } catch (error) {
            console.log('ℹ️  Collection validation already configured or not supported');
        }

        // Get collection stats
        const stats = await collection.stats();
        console.log(`\n📊 Collection Stats:`);
        console.log(`   Documents: ${stats.count}`);
        console.log(`   Indexes: ${stats.nindexes}`);
        console.log(`   Size: ${(stats.size / 1024 / 1024).toFixed(2)} MB`);

        console.log('\n✅ Audit logging migration completed successfully!');
        console.log('\n📝 Next steps:');
        console.log('   1. Set ADMIN_ACCESS_TOKEN in your environment variables');
        console.log('   2. Test audit logging with trigger operations');
        console.log('   3. Set up log rotation and archival policies');

    } catch (error) {
        console.error('❌ Migration failed:', error);
        process.exit(1);
    } finally {
        await mongoose.disconnect();
    }
}

// Run migration if executed directly
if (require.main === module) {
    runAuditMigration();
}

module.exports = { runAuditMigration };