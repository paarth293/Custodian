import React from 'react';
export const SecurityReview = ({ events }: { events: string[] }) => {
    return (
        <div className="security-review">
            {events.map(e => <div key={e}>{e}</div>)}
            <button>I've reviewed this</button>
        </div>
    );
};
