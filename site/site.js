(function(){
function copy(text,btn){
  var done=function(){var o=btn.textContent;btn.textContent="Copied";setTimeout(function(){btn.textContent=o},1500)};
  if(navigator.clipboard&&navigator.clipboard.writeText){navigator.clipboard.writeText(text).then(done,function(){})}
}
function add(host,getText,cls){
  var b=document.createElement("button");b.type="button";b.textContent="Copy";if(cls)b.className=cls;
  b.addEventListener("click",function(){copy(getText(),b)});host.appendChild(b);
}
document.querySelectorAll(".addr").forEach(function(a){
  var c=a.querySelector("code");add(a,function(){return c.textContent});
});
document.querySelectorAll("pre").forEach(function(p){
  p.classList.add("has-copy");add(p,function(){return p.textContent.trim()},"copy");
});
})();
