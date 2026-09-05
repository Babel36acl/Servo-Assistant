// Keep groups contiguous: crossing a gap can touch an unsupported device register.
export function addressGroups<T extends { address: number }>(definitions: T[], maximum: number): T[][] {
  if (!Number.isInteger(maximum) || maximum < 1 || maximum > 100) throw new Error("读取上限须为 1..100");
  const groups: T[][] = [];
  for (const item of [...definitions].sort((a, b) => a.address - b.address)) {
    const last = groups[groups.length - 1];
    if (last && last.length < maximum && last[last.length - 1].address + 1 === item.address) last.push(item);
    else groups.push([item]);
  }
  return groups;
}
