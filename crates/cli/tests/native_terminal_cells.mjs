// Replay the cursor-addressed terminal writes used by the native renderer.
// Assertions inspect the final cells, not concatenated differential output.
export function terminalCells(input, cols=80, rows=24) {
  const cells=Array.from({length:rows},()=>Array(cols).fill(' '));
  let x=0,y=0;
  for(let index=0;index<input.length;) {
    if(input[index]==='\x1b') {
      if(input[index+1]==='[') {
        const match=/^\x1b\[([0-?]*)([ -/]*)([@-~])/.exec(input.slice(index));
        if(!match) break;
        index+=match[0].length;
        const values=match[1].split(';').map(value=>Number(value)||0);
        const count=values[0]||1;
        switch(match[3]) {
          case 'H': case 'f': y=(values[0]||1)-1;x=(values[1]||1)-1;break;
          case 'A': y-=count;break; case 'B': y+=count;break;
          case 'C': x+=count;break; case 'D': x-=count;break;
          case 'G': x=count-1;break; case 'd': y=count-1;break;
          case 'J': if(values[0]===2) for(const row of cells) row.fill(' ');break;
          case 'K': if(y>=0&&y<rows) cells[y].fill(' ',values[0]===2?0:x);break;
        }
        x=Math.max(0,Math.min(cols-1,x));y=Math.max(0,Math.min(rows-1,y));
        continue;
      }
      if([']','P','_','^','X'].includes(input[index+1])) {
        let end=index+2;
        while(end<input.length && input[end]!=='\x07' && !(input[end]==='\x1b'&&input[end+1]==='\\')) end++;
        index=end+(input[end]==='\x07'?1:2);continue;
      }
      index+=2;continue;
    }
    const ch=String.fromCodePoint(input.codePointAt(index));index+=ch.length;
    if(ch==='\r') {x=0;continue;} if(ch==='\n') {y=Math.min(rows-1,y+1);continue;}
    if(ch<' ') continue;
    const wide=/[\u1100-\u115f\u2329-\u232a\u2e80-\ua4cf\uac00-\ud7a3\uf900-\ufaff\ufe10-\ufe6f\uff00-\uff60\uffe0-\uffe6\u{1f300}-\u{1faff}]/u.test(ch);
    if(x<cols&&y<rows) cells[y][x]=ch;
    if(wide&&x+1<cols) cells[y][x+1]='';
    x+=wide?2:1;
  }
  return cells.map(row=>row.join(''));
}
