define(['handlebars'], function(Handlebars) {

this["MinMaximizer"] = this["MinMaximizer"] || {};
this["MinMaximizer"]["templates"] = this["MinMaximizer"]["templates"] || {};

this["MinMaximizer"]["templates"]["modal"] = Handlebars.template({"compiler":[7,">= 4.0.0"],"main":function(container,depth0,helpers,partials,data) {
    var alias1=container.lambda, alias2=container.escapeExpression;

  return "<!-- Modal -->\n<div class=\"modal mymodal\" id=\"minmaximizer-modal\" role=\"dialog\">\n  <div class=\"modal-dialog\">\n  \n    <!-- Modal content-->\n    <div class=\"modal-content\">\n      <div class=\"modal-header\" style=\"padding:35px 50px;\">\n        <button type=\"button\" class=\"close\" data-dismiss=\"modal\" id=\"minmaximizer-button-close\"> <i class='glyphicon glyphicon-remove'></i> </button>   \n        <button class=\"close modalMinimize\" id=\"minmaximizer-button-toggle\"> <i class='glyphicon glyphicon-minus'></i> </button> \n\n        <h4 class=\"modal-title\">"
    + alias2(alias1((depth0 != null ? depth0.title : depth0), depth0))
    + "</h4>\n      </div>\n\n      <div class=\"modal-body\"  style=\"padding:10px 10px;\">\n        "
    + alias2(alias1((depth0 != null ? depth0.body : depth0), depth0))
    + "\n      </div>\n\n      <div class=\"modal-footer\"  style=\"padding:10px 10px;\">\n        "
    + alias2(alias1((depth0 != null ? depth0.footer : depth0), depth0))
    + "\n      </div>\n\n    </div>      \n  </div>\n</div>\n";
},"useData":true});

return this["MinMaximizer"];

});