
export const SafeName = ({ name }: { name: string }) => {
    return <span className="safename" title={name}>{name}</span>;
};
