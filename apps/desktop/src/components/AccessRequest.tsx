import React from 'react';
export const AccessRequest = ({ reasonCode }: { reasonCode: string }) => {
    return <div className="access-request">Request: {reasonCode}</div>;
};
